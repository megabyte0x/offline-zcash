import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Offline Zcash — Basecamp ui_qml port of crates/ozm-node/src/ui.html.
// Every action goes through the core module via the host-injected `logos`
// bridge (LogosQmlBridge): callModuleAsync for methods, onModuleEvent +
// moduleEventReceived for the module's `node_event` event.
Rectangle {
    id: root
    width: 1100
    height: 760
    color: bg

    readonly property string moduleName: "zcash_offline_module"

    // Palette (same as the web UI and the Organiser/Gateway apps).
    readonly property color bg: "#0f1115"
    readonly property color card: "#181b22"
    readonly property color line: "#2a2f3a"
    readonly property color fg: "#e7e9ee"
    readonly property color mut: "#9aa3b5"
    readonly property color ok: "#3fb950"
    readonly property color warn: "#e3a72f"
    readonly property color info: "#58a6ff"
    readonly property color bad: "#ff6b63"
    readonly property color acc: "#f4b728"
    readonly property color field: "#0c0e12"
    readonly property string mono: "monospace"

    property var st: null          // status()
    property var pays: ({ payments: [], refusals: [], gateway: [] })
    property bool paysLoaded: false
    property string reachError: ""
    property string lastEvent: ""
    property var openHistory: ({})   // txid -> true; kept here so a refresh does not collapse rows
    property bool refreshing: false
    readonly property bool joined: st !== null && st.joined === true
    readonly property bool running: st !== null && st.running === true
    readonly property bool showSetup: st !== null && (!joined || !running)
    readonly property string network: st && st.network ? st.network : ""
    readonly property string addrPrefix: root.network === "testnet" ? "utest1" : root.network === "mainnet" ? "u1" : root.network === "regtest" ? "uregtest1" : ""
    readonly property bool narrow: width < 640

    // ── Bridge helpers ──────────────────────────────────────────────────
    // A module method returning a JSON QString reaches QML as a JSON string
    // literal (logos-protocol serializes the QVariant), so unwrap twice.
    function unwrap(payload) {
        var v = payload
        for (var i = 0; i < 3 && typeof v === "string"; i++) {
            try { v = JSON.parse(v) } catch (e) { return { error: String(v) } }
        }
        if (v && typeof v === "object" && v.success !== undefined && v.value !== undefined && v.error !== undefined)
            return v.success ? unwrap(v.value) : { error: String(v.error) }   // LogosResult
        if (v && typeof v === "object" && v.error !== undefined && v.message) return { error: String(v.error) + " (" + (v.method || "call") + "): " + v.message }   // LogosQmlBridge CallError detail
        return (v === null || v === undefined) ? { error: "empty reply" } : v
    }
    function call(method, args, cb, timeoutMs) {
        if (typeof logos === "undefined" || !logos) { cb({ error: "Logos bridge not available" }); return }
        logos.callModuleAsync(moduleName, method, args, function (payload) { cb(unwrap(payload)) },
                              timeoutMs === undefined ? 30000 : timeoutMs)
    }

    function zec(z) {
        var s = (Number(z || 0) / 1e8).toFixed(8).replace(/0+$/, "").replace(/\.$/, "")
        return s + " ZEC"
    }
    function hms(ms) { return ms ? new Date(ms).toLocaleTimeString(Qt.locale(), "HH:mm:ss") : "" }
    // [label, tone, explanation]; same table as the web UI. Chain results stay gateway-reported.
    readonly property var states: ({
        "built": ["Built", "pending", "Proof built on this device; not sent yet."],
        "sent": ["Sent over the mesh", "pending", "Waiting for the payee device to receive it."],
        "delivered": ["Delivered", "pending", "The payee device received it; waiting for its receipt."],
        "accepted-pending": ["Accepted, pending", "pending", "An invited payee accepted it before inclusion. It is not on chain yet."],
        "awaiting-inclusion": ["Awaiting inclusion", "pending", "Checked on this device; waiting for the gateway to report inclusion."],
        "submitted": ["Submitted", "pending", "A gateway submitted it; not included yet."],
        "submitted, no payee receipt": ["Submitted · no payee receipt", "pending", "Sent to a gateway for an address with no mesh payee. No receipt will come."],
        "in-mempool": ["In mempool", "pending", "In the gateway's mempool (gateway-reported)."],
        "retrying": ["Retrying", "warn", "The gateway hit a retryable error and will resubmit."],
        "included": ["Included · gateway-reported", "included", "The gateway reports fewer than 10 confirmations. Not independently proved."],
        "mined": ["Mined · gateway-reported", "ok", "The gateway reports it mined. Not independently proved."],
        "expired": ["Expired", "bad", "The gateway reports it expired. Locked notes return after your sync passes the expiry height."],
        "conflict": ["Conflict", "bad", "The gateway reports another transaction spending the same note."],
        "gateway-rejected": ["Rejected by gateway", "bad", "The gateway refused to submit it. See history for the reason."],
        "refused, may still mine until expiry": ["Refused by payee", "bad", "The payee refused it. It may still be mined until it expires."]
    })
    function stateInfo(s) { return states[s] || [String(s), "pending", ""] }
    function toneColor(t) { return t === "ok" ? ok : t === "included" ? info : t === "bad" ? bad : warn }
    function direction(d) { return d === "sent" ? "Sent" : d === "received" ? "Received" : d === "prepare" ? "Prepare (split to self)" : String(d) }
    function amountError(v) {
        if (!/^\d+(\.\d{1,8})?$/.test(String(v).trim())) return "Enter an amount like 0.01 (up to 8 decimals)."
        if (Number(v) <= 0) return "Enter an amount greater than 0."
        return ""
    }

    function refresh() {
        if (refreshing) return
        refreshing = true
        call("status", [], function (s) {
            if (s.error) { reachError = s.error; refreshing = false; return }
            reachError = ""
            st = s
            if (!s.joined || !s.running) { refreshing = false; return }
            call("list_payments", [], function (p) {
                refreshing = false
                if (p.error) return
                paysLoaded = true
                pays = { payments: (p.payments || []).slice().sort(function (a, b) { return b.created_ms - a.created_ms }),
                         refusals: p.refusals || [], gateway: p.gateway || [] }
            })
        }, 10000)
    }

    Component.onCompleted: {
        if (typeof logos !== "undefined" && logos)
            logos.onModuleEvent(moduleName, "node_event")
        refresh()
    }
    Connections {
        target: (typeof logos !== "undefined") ? logos : null
        ignoreUnknownSignals: true
        function onModuleEventReceived(module, eventName, data) {
            if (module !== root.moduleName) return
            var ev = (data && data.length > 0) ? String(data[0]) : eventName
            root.lastEvent = ev + " · " + root.hms(Date.now())
            root.refresh()
        }
    }
    Timer { interval: 3000; running: true; repeat: true; onTriggered: root.refresh() }
    Popup {
        id: leaveConfirm
        parent: Overlay.overlay; anchors.centerIn: parent
        modal: true; focus: true; padding: 20
        width: Math.min(root.width - 40, 480)
        background: Rectangle { color: root.card; border.color: root.line; radius: 10 }
        contentItem: ColumnLayout {
            spacing: 10
            Text { text: "Leave this session?"; color: root.fg; font.pixelSize: 18; font.bold: true }
            Muted { text: "The node stops. This device's keys, wallet and payment history move to a backup folder next to it; nothing is deleted. Afterwards this device can start without an invite or join a new one, on either network. Starting again without an invite on the same network restores this device's own wallet: same address and balance. An invite's wallet stays in the backup." }
            RowLayout { Layout.fillWidth: true; spacing: 10
                Btn { secondary: true; text: "Cancel"; onClicked: leaveConfirm.close() }
                Btn {
                    objectName: "confirmLeaveButton"; text: "Leave and keep a backup"
                    onClicked: {
                        leaveConfirm.close(); setup.busy = true; setup.failed = false; setup.msg = "Leaving…"
                        root.call("leave", [], function (r) {
                            setup.busy = false; setup.failed = !!r.error
                            setup.msg = r.error ? "Could not leave: " + r.error : r.message
                            root.paysLoaded = false; root.pays = { payments: [], refusals: [], gateway: [] }
                            root.refresh()
                        }, 60000)
                    }
                } }
        }
    }

    // ── Reusable bits ───────────────────────────────────────────────────
    component Pill: Rectangle {
        property alias text: t.text
        property color textColor: root.mut
        radius: height / 2
        color: root.card
        border.color: root.line
        implicitHeight: t.implicitHeight + 6
        implicitWidth: Math.min(t.implicitWidth + 18, root.width - 40)
        Text { id: t; anchors.fill: parent; anchors.leftMargin: 9; anchors.rightMargin: 9; verticalAlignment: Text.AlignVCenter
               elide: Text.ElideRight; color: parent.textColor; font.pixelSize: 12; textFormat: Text.PlainText }
    }
    component Card: Rectangle {
        id: cardRoot
        default property alias content: col.data
        property string title: ""
        color: root.card
        border.color: root.line
        radius: 10
        implicitHeight: col.implicitHeight + 32
        Accessible.role: Accessible.Grouping
        Accessible.name: title
        ColumnLayout {
            id: col
            anchors.fill: parent
            anchors.margins: 16
            spacing: 6
            Text {
                text: cardRoot.title.toUpperCase()
                color: root.mut; font.pixelSize: 13; font.letterSpacing: 0.8
                Layout.bottomMargin: 4
            }
        }
    }
    component Btn: Button {
        id: b
        property bool secondary: false
        Layout.fillWidth: true
        Layout.topMargin: 8
        hoverEnabled: true
        padding: 10
        Accessible.name: text
        Keys.onReturnPressed: if (enabled) clicked()
        contentItem: Text {
            text: b.text; color: b.secondary ? root.fg : "#111"; font.bold: true; font.pixelSize: 14
            horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
            wrapMode: Text.Wrap
        }
        background: Rectangle {
            implicitHeight: 40; radius: 8
            color: b.secondary ? (b.hovered && b.enabled ? "#222733" : "transparent") : root.acc
            opacity: b.enabled ? (b.down ? 0.8 : 1) : 0.45
            border.width: b.visualFocus ? 2 : 1
            border.color: b.visualFocus ? root.fg : b.secondary ? root.line : "transparent"
        }
    }
    component Field: TextField {
        id: f
        property bool invalid: false
        Layout.fillWidth: true
        color: root.fg
        selectByMouse: true
        placeholderTextColor: "#7d8699"
        font.pixelSize: 14
        padding: 9
        background: Rectangle { radius: 8; color: root.field; border.width: f.activeFocus ? 2 : 1
                                border.color: f.invalid ? root.bad : f.activeFocus ? root.acc : root.line }
    }
    component Label2: Text { color: root.mut; font.pixelSize: 14; Layout.topMargin: 6; wrapMode: Text.Wrap; Layout.fillWidth: true }
    component Muted: Text { color: root.mut; font.pixelSize: 14; wrapMode: Text.Wrap; Layout.fillWidth: true; textFormat: Text.PlainText }
    component Note: Rectangle {
        property alias text: nt.text
        property color tone: root.warn
        Layout.fillWidth: true
        visible: nt.text !== ""
        radius: 8; color: Qt.rgba(tone.r, tone.g, tone.b, 0.08); border.color: Qt.rgba(tone.r, tone.g, tone.b, 0.45)
        implicitHeight: nt.implicitHeight + 16
        Accessible.role: Accessible.AlertMessage
        Accessible.name: nt.text
        Text { id: nt; anchors.fill: parent; anchors.margins: 8; color: parent.tone; font.pixelSize: 13; wrapMode: Text.Wrap; textFormat: Text.PlainText }
    }
    component Msg: Muted {
        property bool failed: false
        visible: text !== ""
        color: failed ? root.bad : root.mut
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
    component Toggle: Text {
        property bool open: false
        signal toggled()
        color: root.fg; font.pixelSize: 14; Layout.topMargin: 8
        text: (open ? "▾ " : "▸ ") + label
        property string label: ""
        Accessible.role: Accessible.Button
        Accessible.name: label
        activeFocusOnTab: true
        Keys.onSpacePressed: { open = !open; toggled() }
        Keys.onReturnPressed: { open = !open; toggled() }
        Rectangle { anchors.fill: parent; anchors.margins: -3; color: "transparent"; radius: 4; border.color: root.fg; visible: parent.activeFocus }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { parent.open = !parent.open; parent.toggled() } }
    }

    // ── Layout ──────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Header
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: hdr.implicitHeight + 28
            color: root.bg
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: root.line }
            Flow {
                id: hdr
                anchors.fill: parent
                anchors.margins: 14
                anchors.leftMargin: 20
                spacing: 10
                Rectangle { width: 24; height: 24; radius: 12; color: root.acc; Accessible.ignored: true
                    Text { anchors.centerIn: parent; text: "Z"; color: "#111"; font.pixelSize: 15; font.bold: true } }
                Text {
                    textFormat: Text.StyledText
                    text: "Offline Zcash <font color='#9aa3b5'>"
                          + (root.st && root.st.version ? "v" + root.st.version : "") + "</font>"
                    color: root.fg; font.pixelSize: 18; font.bold: true
                }
                Pill {
                    text: root.reachError !== "" ? "service not answering"
                          : root.st ? (root.st.name || "not joined") + (root.st.role ? " · " + root.st.role : "") + " · " + root.network : "Connecting…"
                    textColor: root.reachError !== "" ? root.bad : root.mut
                }
                Pill {
                    objectName: "regtestBadge"
                    visible: root.network === "regtest"
                    text: "Regtest session (local ths chain) · Leave session to switch to testnet"
                    textColor: root.warn
                }
                Pill {
                    visible: root.st !== null
                    text: !root.running ? "stopped" : root.st.syncing ? "syncing · height " + (root.st.wallet ? root.st.wallet.tip : 0) + (root.st.chain_tip ? " of " + root.st.chain_tip : "")
                          : root.st.ready ? "ready · " + (root.st.online ? "gateway / online" : "gateway via mesh") : root.st.online ? "gateway / online"
                          : root.st.selected_gateway ? "offline · gateway via mesh" : "offline · no gateway yet"
                    textColor: !root.running ? root.mut : (root.st.online || root.st.selected_gateway) ? root.ok : root.warn
                }
                Pill {
                    visible: root.running
                    text: "mesh neighbours: " + (root.st && root.st.neighbor_links && root.st.neighbor_links.length
                          ? root.st.neighbor_links.map(function (n) { return n.name + " (" + n.kind.toUpperCase() + ")" }).join(", ") : "none")
                }
            }
        }

        ScrollView {
            id: scroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                width: scroll.availableWidth
                spacing: 16

                Item { implicitHeight: 4 }

                // ── Not reachable / loading ──
                Card {
                    visible: root.st === null || root.reachError !== ""
                    title: root.reachError !== "" ? "Service not answering" : "Loading"
                    Layout.fillWidth: true
                    Layout.maximumWidth: 720
                    Layout.leftMargin: 20; Layout.rightMargin: 20
                    Muted {
                        text: root.reachError !== ""
                              ? "The Offline Zcash service did not answer: " + root.reachError + ". "
                                + (/not connected|not reachable|no such module|not loaded/i.test(root.reachError) ? "Install or load the Offline Zcash service in Manage Packages. " : "It may be busy. ")
                                + "This page retries every 3 seconds."
                              : "Connecting to zcash_offline_module…"
                        color: root.reachError !== "" ? root.bad : root.mut
                    }
                }

                // ── Setup (not joined / not running) ──
                Card {
                    id: setup
                    visible: root.showSetup
                    title: root.joined ? "Start this device" : "Get started"
                    Layout.fillWidth: true
                    Layout.maximumWidth: 720
                    Layout.leftMargin: 20; Layout.rightMargin: 20
                    property string msg: ""
                    property bool failed: false
                    property bool busy: false

                    Muted {
                        visible: root.joined
                        text: "This device has joined as " + (root.st ? root.st.name + " (" + (root.st.role ? root.st.role + ", " : "") + root.network + ")" : "") + ". Start it to reconnect to the mesh."
                    }
                    Text { visible: !root.joined; text: "No invite needed"; color: root.fg; font.pixelSize: 16; font.bold: true }
                    Muted {
                        visible: !root.joined
                        text: "1. Start creates your keys on this device.\n2. It finds a gateway on the local network and syncs through it.\n3. Share your address to get paid in TAZ (public testnet test coins)."
                    }
                    Muted {
                        visible: !root.joined
                        font.pixelSize: 12
                        text: useRegtest.checked ? "Network: local Regtest (developers). Needs the operator's ths chain and a Regtest gateway on the mesh." : "Network: public testnet (TAZ test coins). Needs a testnet gateway on the mesh."
                    }
                    Toggle { id: regtestToggle; visible: !root.joined; label: "Advanced (local Regtest for developers)" }
                    ColumnLayout {
                        visible: !root.joined && regtestToggle.open
                        Layout.fillWidth: true
                        spacing: 4
                        Switch {
                            id: useRegtest
                            objectName: "useRegtest"
                            text: "Use a local Regtest chain instead of testnet"
                            palette.windowText: root.fg
                            palette.text: root.fg
                            checked: false
                            Accessible.name: text
                        }
                        Muted { font.pixelSize: 12; text: "Only for development: needs a local ths chain run by the session operator. Leave this off for testnet." }
                    }

                    Toggle { id: inviteToggle; visible: !root.joined; label: "I have an organiser invite (optional)" }
                    Rectangle {
                        visible: !root.joined && inviteToggle.open
                        Layout.fillWidth: true
                        implicitHeight: 120
                        radius: 8; color: root.field; border.color: invite.activeFocus ? root.acc : root.line; border.width: invite.activeFocus ? 2 : 1
                        ScrollView {
                            anchors.fill: parent
                            anchors.margins: 1
                            TextArea {
                                id: invite
                                objectName: "inviteInput"
                                color: root.fg
                                font.family: root.mono; font.pixelSize: 12
                                wrapMode: TextEdit.WrapAnywhere
                                selectByMouse: true
                                placeholderText: "Paste the invite JSON from your organiser"
                                placeholderTextColor: "#7d8699"
                                background: null
                                Accessible.name: "Invite JSON"
                            }
                        }
                    }
                    Muted { visible: !root.joined && inviteToggle.open; font.pixelSize: 12; text: "An invite sets your name, network and trusted gateway; the Regtest option is then ignored." }

                    Toggle { id: advToggle; label: "Connection options" }
                    ColumnLayout {
                        visible: advToggle.open
                        Layout.fillWidth: true
                        spacing: 4
                        Label2 { text: "Dial address (optional)" }
                        Field { id: dial; objectName: "dialInput"; placeholderText: "operator-ip:7777, comma-separated"; Accessible.name: "Dial address" }
                        Muted { font.pixelSize: 12; text: "Use when local discovery is blocked, for example macOS Local Network permission." }
                        Label2 { text: "Listen address (optional)" }
                        Field { id: listenAddr; objectName: "listenInput"; placeholderText: "default 0.0.0.0:7777"; Accessible.name: "Listen address" }
                        Switch {
                            id: bluetooth
                            objectName: "bluetoothToggle"
                            text: "Bluetooth LE (Linux only, experimental)"
                            palette.windowText: root.fg
                            palette.text: root.fg
                            checked: false
                            Accessible.name: text
                        }
                        Muted { font.pixelSize: 12; text: "Needs Bluetooth on and a BlueZ adapter that can advertise. Not yet tested on real radios." }
                    }
                    Btn {
                        objectName: "startNodeButton"
                        text: setup.busy ? "Working…" : (root.joined ? "Start" : invite.text.trim() === "" ? "Start without invite" : "Join with invite and start")
                        enabled: !setup.busy
                        onClicked: {
                            setup.busy = true; setup.failed = false
                            var peers = dial.text.split(/[\s,]+/).filter(function (s) { return s !== "" })
                            var opts = peers.length ? { dial: peers } : {}
                            opts.network = useRegtest.checked ? "regtest" : "testnet"
                            if (listenAddr.text.trim() !== "") opts.listen = listenAddr.text.trim()
                            opts.ble = bluetooth.checked
                            var startNode = function () {
                                setup.msg = "Starting… creating keys and opening the mesh."
                                root.call("start", [JSON.stringify(opts)], function (r) {
                                    setup.busy = false
                                    setup.failed = !!r.error
                                    setup.msg = r.error ? "Could not start: " + r.error : "Started"
                                    root.refresh()
                                }, 60000)
                            }
                            if (root.joined || invite.text.trim() === "") { startNode(); return }
                            setup.msg = "Joining…"
                            root.call("join", [invite.text.trim()], function (r) {
                                if (r.error) { setup.busy = false; setup.failed = true; setup.msg = "Invite not accepted: " + r.error + ". Check that you pasted the whole file."; return }
                                setup.msg = r.message || "Joined"
                                startNode()
                            })
                        }
                    }
                    Msg { text: setup.msg; failed: setup.failed }
                }

                // ── Main grid ──
                GridLayout {
                    id: grid
                    visible: root.running
                    Layout.fillWidth: true
                    Layout.leftMargin: root.narrow ? 12 : 20; Layout.rightMargin: root.narrow ? 12 : 20
                    Layout.maximumWidth: 1200
                    columns: Math.max(1, Math.min(3, Math.floor((scroll.availableWidth - 40) / 336)))
                    columnSpacing: 16; rowSpacing: 16

                    // Wallet
                    Card {
                        id: wallet
                        title: "Wallet"
                        Layout.fillWidth: true; Layout.fillHeight: true; Layout.preferredWidth: 320
                        property string msg: ""
                        property bool failed: false
                        property bool busy: false
                        Text {
                            objectName: "balance"
                            text: root.st && root.st.wallet ? root.zec(root.st.wallet.spendable) : "Loading…"
                            color: root.fg; font.pixelSize: 26; font.weight: Font.DemiBold
                            Accessible.name: "Spendable balance " + text
                        }
                        Muted {
                            text: root.st && root.st.wallet ? "spendable · " + root.zec(root.st.wallet.locked) + " locked in pending payments · "
                                            + root.st.wallet.notes + " notes · balance as of height " + root.st.wallet.tip : ""
                        }
                        Msg { text: root.st && root.st.syncing ? "Syncing in the background" + (root.st.chain_tip ? ": height " + root.st.wallet.tip + " of " + root.st.chain_tip : "") + ". Available notes can still be paid or prepared." : root.st && root.st.sync_error ? "Last sync failed: " + root.st.sync_error : ""
                              failed: !!(root.st && !root.st.syncing && root.st.sync_error) }
                        Rectangle {
                            Layout.fillWidth: true; Layout.topMargin: 4
                            radius: 8; color: "transparent"; border.color: root.line
                            implicitHeight: cv.implicitHeight + 16
                            ColumnLayout {
                                id: cv
                                anchors.fill: parent; anchors.margins: 8
                                spacing: 2
                                Muted { font.pixelSize: 12; text: "Chain view (gateway-reported, not independently proved)" }
                                Text {
                                    Layout.fillWidth: true; wrapMode: Text.Wrap; color: root.fg; font.pixelSize: 13; textFormat: Text.PlainText
                                    text: root.st && root.st.selected_gateway
                                          ? "Gateway " + root.st.selected_gateway.name + " · " + root.st.selected_gateway.hops + " hops · tip " + root.st.selected_gateway.tip + " · mesh ID " + root.st.selected_gateway.peer_id
                                          : root.st && root.st.online ? "This device is the gateway (chain service)." : "Finding a gateway over the mesh…"
                                }
                                Muted {
                                    font.pixelSize: 12
                                    text: !root.st ? "" : !root.st.single_source ? "Cross-checked against " + (root.st.chain_sources || []).length + " chain sources."
                                          : (root.st.chain_sources || []).length ? "Single chain source: not cross-checked." : "No chain source connected yet."
                                }
                            }
                        }
                        Note { text: root.st ? root.st.gateway_warning || "" : "" }
                        Btn {
                            secondary: true
                            text: wallet.busy ? "Refreshing…" : "Refresh balance (optional)"
                            Layout.alignment: Qt.AlignLeft
                            enabled: !wallet.busy
                            onClicked: {
                                wallet.busy = true; wallet.failed = false; wallet.msg = "Syncing…"
                                root.call("sync_now", [], function (r) {
                                    wallet.busy = false; wallet.failed = !!r.error
                                    wallet.msg = r.error ? "Sync failed: " + r.error : "Syncing in the background; the balance updates as blocks arrive."
                                    root.refresh()
                                })
                            }
                        }
                        Btn {
                            secondary: true
                            visible: root.st !== null && !root.st.online && root.network === "regtest"
                            text: "Get test funds (5 × 0.1 ZEC)"
                            enabled: !wallet.busy
                            onClicked: {
                                wallet.busy = true; wallet.failed = false; wallet.msg = "Asking the gateway for test funds…"
                                root.call("request_funds", [5, "0.1"], function (r) {
                                    wallet.busy = false; wallet.failed = !!r.error
                                    wallet.msg = r.error ? "Test funds not requested: " + r.error
                                        : "Asked " + r.requested_from + ". Funds appear after they are mined (about a minute)."
                                })
                            }
                        }
                        Muted { visible: root.network === "testnet"; font.pixelSize: 12; text: "Testnet funds: faucet.testnet.valargroup.dev (0.125 TAZ / 24 h; the service may fail). Gateways do not hand out testnet funds." }
                        Msg { text: wallet.msg; failed: wallet.failed }
                        Item { Layout.fillHeight: true }
                    }

                    // Pay offline
                    Card {
                        id: pay
                        title: "Pay offline"
                        Layout.fillWidth: true; Layout.fillHeight: true; Layout.preferredWidth: 320
                        property string msg: ""
                        property bool failed: false
                        property string feeNote: ""
                        property bool busy: false
                        readonly property bool wrongPrefix: root.addrPrefix !== "" && to.text.trim() !== "" && to.text.trim().indexOf(root.addrPrefix) !== 0
                        readonly property string amountErr: root.amountError(amt.text)
                        Label2 { text: "Recipient Zcash address" }
                        Field {
                            id: to
                            objectName: "recipientAddress"
                            placeholderText: root.addrPrefix !== "" ? root.addrPrefix + "…" : "Unified address"
                            invalid: pay.wrongPrefix
                            Accessible.name: "Recipient Zcash address"
                            Accessible.description: hint.text
                        }
                        Muted {
                            id: hint
                            font.pixelSize: 12
                            color: pay.wrongPrefix ? root.warn : root.mut
                            text: pay.wrongPrefix ? "This does not look like a " + root.network + " unified address (expected " + root.addrPrefix + "…)."
                                                  : "Paste the unified address the recipient shared with you."
                        }
                        Label2 { text: "Amount (ZEC)" }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Field {
                                id: amt; objectName: "amountInput"; text: "0.01"
                                inputMethodHints: Qt.ImhFormattedNumbersOnly
                                invalid: pay.amountErr !== ""
                                Accessible.name: "Amount in ZEC"
                                onTextEdited: pay.feeNote = ""
                                onAccepted: if (payBtn.enabled) payBtn.clicked()
                            }
                            Btn {
                                secondary: true
                                objectName: "maxButton"; text: "Max"
                                Accessible.name: "Use maximum spendable amount"
                                Layout.fillWidth: false; Layout.topMargin: 0; Layout.preferredWidth: 72
                                enabled: !pay.busy
                                onClicked: root.call("max_payment", [], function (r) {
                                    if (r.error) { pay.feeNote = "Cannot compute Max: " + r.error; return }
                                    amt.text = r.amount
                                    pay.feeNote = "Max after the builder fee of " + root.zec(r.fee_zat) + ". Changes if your spendable notes change."
                                })
                            }
                        }
                        Muted {
                            font.pixelSize: 12
                            visible: text !== ""
                            color: pay.amountErr !== "" || pay.feeNote.indexOf("Cannot") === 0 ? root.bad : root.mut
                            text: pay.amountErr !== "" ? pay.amountErr : pay.feeNote
                        }
                        Btn {
                            id: payBtn
                            text: pay.busy ? "Proving on this device…" : "Build, prove and send over the mesh"
                            enabled: !pay.busy && to.text.trim() !== "" && pay.amountErr === ""
                            onClicked: {
                                pay.busy = true; pay.failed = false; pay.msg = "Building and proving. This can take a few seconds."
                                root.call("pay", [to.text.trim(), amt.text.trim()], function (r) {
                                    pay.busy = false; pay.failed = !!r.error
                                    pay.msg = r.error ? "Payment not sent: " + r.error
                                        : "Sent " + String(r.txid || "").slice(0, 16) + "… · " + root.stateInfo(r.state)[0] + ". Follow it under Payments."
                                    root.refresh()
                                }, 180000)
                            }
                        }
                        Msg { objectName: "payResult"; text: pay.msg; failed: pay.failed }
                        Muted { font.pixelSize: 12; text: "A known mesh payee answers with a receipt. An unknown address is still submitted, but you get no payee receipt. Included and mined are reported by the gateway." }
                        Item { Layout.fillHeight: true }
                    }

                    // Receive
                    Card {
                        title: "Receive"
                        Layout.fillWidth: true; Layout.fillHeight: true; Layout.preferredWidth: 320
                        TextEdit {
                            id: addr
                            objectName: "address"
                            Layout.fillWidth: true
                            readOnly: true; selectByMouse: true
                            text: root.st && root.st.address ? root.st.address : ""
                            color: root.fg; selectionColor: root.acc; selectedTextColor: "#111"
                            font.family: root.mono; font.pixelSize: 12
                            wrapMode: TextEdit.WrapAnywhere
                            textFormat: TextEdit.PlainText
                            Accessible.name: "Your address"
                        }
                        Muted { visible: addr.text === ""; text: "No address yet." }
                        Btn {
                            id: copyBtn
                            secondary: true
                            property bool copied: false
                            text: copied ? "Copied" : "Copy your address"
                            enabled: addr.text !== ""
                            onClicked: { addr.selectAll(); addr.copy(); addr.deselect(); copied = true; copyReset.restart() }
                            Timer { id: copyReset; interval: 1500; onTriggered: copyBtn.copied = false }
                        }
                        Muted { font.pixelSize: 12; text: "Open or unknown payers are not accepted while pending. Wait for gateway-reported inclusion; it is not independent chain verification." }
                        Item { Layout.fillHeight: true }
                    }

                    // Prepare for offline
                    Card {
                        id: prep
                        title: "Prepare for offline"
                        Layout.fillWidth: true
                        Layout.columnSpan: grid.columns
                        property string msg: ""
                        property bool failed: false
                        property bool busy: false
                        Muted { text: "Split one mined note into equal notes so you can make several payments without a connection." }
                        Muted { font.pixelSize: 13; text: "1. Prepare while a gateway is reachable.\n2. Wait until the split shows Mined (10 confirmations on testnet).\n3. Sync, then go offline. Never spend unmined change." }
                        GridLayout {
                            Layout.fillWidth: true
                            columns: root.narrow ? 1 : 3
                            columnSpacing: 12; rowSpacing: 4
                            ColumnLayout {
                                Layout.fillWidth: true; spacing: 2
                                Label2 { text: "Number of notes" }
                                Field { id: splitCount; text: "10"; inputMethodHints: Qt.ImhDigitsOnly; Accessible.name: "Prepare note count"
                                        invalid: !(Number(text) >= 2 && Number(text) <= 100 && /^\d+$/.test(text)) }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true; spacing: 2
                                Label2 { text: "ZEC per note" }
                                Field { id: splitValue; text: "0.0101"; inputMethodHints: Qt.ImhFormattedNumbersOnly; Accessible.name: "Amount per prepared note"
                                        invalid: root.amountError(text) !== "" }
                            }
                            Btn {
                                Layout.alignment: Qt.AlignBottom
                                Layout.topMargin: 0
                                secondary: true
                                objectName: "prepareButton"
                                text: prep.busy ? "Proving the split…" : "Prepare for offline"
                                enabled: !prep.busy && !splitCount.invalid && !splitValue.invalid
                                onClicked: {
                                    prep.busy = true; prep.failed = false; prep.msg = "Proving the split…"
                                    // The bridge refuses a JS double for the i64 note count; `| 0` passes an integer.
                                    root.call("prepare_offline", [Number(splitCount.text) | 0, splitValue.text.trim()], function (r) {
                                        prep.busy = false; prep.failed = !!r.error
                                        prep.msg = r.error ? "Not prepared: " + r.error
                                            : "Split " + String(r.txid || "").slice(0, 16) + "… submitted. Wait for Mined (10 confirmations on testnet), then sync before going offline."
                                        root.refresh()
                                    }, 120000)
                                }
                            }
                        }
                        Muted {
                            font.pixelSize: 12
                            color: splitCount.invalid || splitValue.invalid ? root.bad : root.mut
                            text: splitCount.invalid ? "Choose between 2 and 100 notes." : splitValue.invalid ? root.amountError(splitValue.text) : "The split fee grows with the number of notes."
                        }
                        Msg { text: prep.msg; failed: prep.failed }
                    }

                    // Payments
                    Card {
                        title: "Payments"
                        Layout.fillWidth: true
                        Layout.columnSpan: grid.columns
                        Repeater {
                            model: root.pays.payments
                            ColumnLayout {
                                id: prow
                                required property var modelData
                                readonly property bool open: root.openHistory[modelData.txid] === true
                                readonly property var info: root.stateInfo(modelData.state)
                                Layout.fillWidth: true
                                spacing: 3
                                Rectangle { Layout.fillWidth: true; height: 1; color: root.line; Layout.bottomMargin: 4 }
                                Flow {
                                    Layout.fillWidth: true
                                    spacing: 12
                                    Text { text: prow.info[0]; color: root.toneColor(prow.info[1]); font.pixelSize: 14; font.bold: true; textFormat: Text.PlainText }
                                    Text { text: root.direction(prow.modelData.direction) + " · " + root.zec(prow.modelData.amount); color: root.fg; font.pixelSize: 14 }
                                    Text { text: root.hms(prow.modelData.created_ms); color: root.mut; font.pixelSize: 13 }
                                }
                                Muted { visible: prow.info[2] !== ""; text: prow.info[2]; font.pixelSize: 12 }
                                Muted { visible: prow.modelData.direction === "sent" && !prow.modelData.receipt_peer && prow.modelData.state !== "submitted, no payee receipt"; text: "No payee receipt"; font.pixelSize: 12 }
                                Text { text: "With " + prow.modelData.counterparty; textFormat: Text.PlainText; color: root.mut; font.pixelSize: 12; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                                TextEdit {
                                    text: prow.modelData.txid; readOnly: true; selectByMouse: true; textFormat: TextEdit.PlainText
                                    color: root.fg; font.family: root.mono; font.pixelSize: 12
                                    wrapMode: TextEdit.WrapAnywhere
                                    Layout.fillWidth: true
                                    Accessible.name: "Transaction ID"
                                }
                                Toggle {
                                    label: "history"
                                    open: prow.open
                                    font.pixelSize: 13; color: root.mut; Layout.topMargin: 0
                                    onToggled: {
                                        var o = Object.assign({}, root.openHistory)
                                        if (prow.open) delete o[prow.modelData.txid]; else o[prow.modelData.txid] = true
                                        root.openHistory = o
                                    }
                                }
                                Repeater {
                                    model: prow.open ? (prow.modelData.history || []) : []
                                    Text {
                                        text: root.hms(modelData.ms) + "  " + modelData.event
                                        textFormat: Text.PlainText
                                        color: root.mut; font.pixelSize: 12; wrapMode: Text.Wrap
                                        Layout.fillWidth: true; Layout.leftMargin: 16
                                    }
                                }
                            }
                        }
                        Muted {
                            visible: root.pays.payments.length === 0
                            text: root.paysLoaded ? "No payments yet. Payments you send or receive appear here with their state." : "Loading…"
                        }
                        Repeater {
                            model: root.pays.refusals
                            Note {
                                tone: root.bad
                                text: "Refused by " + modelData.from + ": " + (modelData.reject ? "code " + modelData.reject.code + ", " + modelData.reject.reason : "no reason")
                                      + (modelData.txid ? " (" + String(modelData.txid).slice(0, 16) + "…)" : "") + ". It may still be mined until it expires."
                            }
                        }
                        Flow {
                            Layout.fillWidth: true; Layout.topMargin: 8
                            spacing: 14
                            Repeater {
                                model: [["pending", "waiting"], ["included", "included (gateway-reported)"], ["ok", "mined (gateway-reported)"], ["bad", "refused, rejected, expired or conflict"]]
                                Text { text: "<font color='" + root.toneColor(modelData[0]) + "'>●</font> " + modelData[1]; textFormat: Text.StyledText; color: root.mut; font.pixelSize: 12 }
                            }
                        }
                    }

                    // Mesh
                    Card {
                        title: "Mesh"
                        Layout.fillWidth: true
                        Layout.columnSpan: grid.columns
                        Text {
                            Layout.fillWidth: true; wrapMode: Text.WrapAnywhere; textFormat: Text.PlainText
                            font.family: root.mono; font.pixelSize: 12; color: root.fg
                            text: root.st && root.st.mesh
                                  ? "peer " + root.st.peer_id + " · frames sent " + root.st.mesh.frames_sent + " / received " + root.st.mesh.frames_received
                                    + " · relayed " + root.st.mesh.packets_relayed + " · stored " + root.st.mesh.stored
                                    + (root.st.mesh.discovered !== undefined ? " · discovered " + root.st.mesh.discovered : "")
                                  : ""
                        }
                        Muted { visible: root.lastEvent !== ""; text: "last event: " + root.lastEvent; font.pixelSize: 12 }
                        Muted {
                            text: "Bluetooth: " + (root.st && root.st.ble
                                  ? (root.st.ble_running ? "on" : "stopped, restart the node") : "off")
                        }
                    }
                }
                Card {
                    visible: root.joined
                    title: "Session"
                    Layout.fillWidth: true
                    Layout.maximumWidth: 720
                    Layout.leftMargin: root.narrow ? 12 : 20; Layout.rightMargin: root.narrow ? 12 : 20
                    Muted { text: "To switch network or use another invite, leave this session. A backup of this device's keys and wallet is kept, and its own (invite-free) wallet comes back on the same network." }
                    Btn { objectName: "leaveSessionButton"; secondary: true; text: "Leave session…"; enabled: !setup.busy; onClicked: leaveConfirm.open() }
                    Muted { visible: !!(root.st && root.st.syncing); font.pixelSize: 12; text: "Leaving now stops the wallet sync; nothing is lost." }
                }
                Item { implicitHeight: 12 }
            }
        }
    }
}
