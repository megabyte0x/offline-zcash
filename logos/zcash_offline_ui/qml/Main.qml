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

    // Palette (same as the web UI).
    readonly property color bg: "#0f1115"
    readonly property color card: "#181b22"
    readonly property color line: "#2a2f3a"
    readonly property color fg: "#e7e9ee"
    readonly property color mut: "#8a93a6"
    readonly property color ok: "#3fb950"
    readonly property color warn: "#d29922"
    readonly property color bad: "#f85149"
    readonly property color acc: "#f4b728"
    readonly property color field: "#0c0e12"
    readonly property string mono: "monospace"

    property var st: null          // status()
    property var pays: ({ payments: [], refusals: [], gateway: [] })
    property string reachError: ""
    property string lastEvent: ""
    property var openHistory: ({})   // txid -> true; kept here so a refresh does not collapse rows
    property bool refreshing: false
    readonly property bool joined: st !== null && st.joined === true
    readonly property bool running: st !== null && st.running === true
    readonly property bool showSetup: st !== null && (!joined || !running)

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
    function stateColor(s) {
        if (s === "mined") return ok
        if (s === "refused" || s === "gateway-rejected") return bad
        if (["accepted-pending", "submitted", "delivered", "sent", "in-mempool", "built"].indexOf(s) >= 0) return warn
        return fg
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

    // ── Reusable bits ───────────────────────────────────────────────────
    component Pill: Rectangle {
        property alias text: t.text
        property color textColor: root.mut
        radius: height / 2
        color: root.card
        border.color: root.line
        implicitHeight: t.implicitHeight + 6
        implicitWidth: t.implicitWidth + 18
        Text { id: t; anchors.centerIn: parent; color: parent.textColor; font.pixelSize: 12 }
    }
    component Card: Rectangle {
        default property alias content: col.data
        property string title: ""
        color: root.card
        border.color: root.line
        radius: 10
        implicitHeight: col.implicitHeight + 32
        ColumnLayout {
            id: col
            anchors.fill: parent
            anchors.margins: 16
            spacing: 6
            Text {
                text: parent.parent.title.toUpperCase()
                color: root.mut; font.pixelSize: 13; font.letterSpacing: 0.8
                Layout.bottomMargin: 4
            }
        }
    }
    component Btn: Button {
        id: b
        Layout.fillWidth: true
        Layout.topMargin: 8
        hoverEnabled: true
        padding: 10
        contentItem: Text {
            text: b.text; color: "#111"; font.bold: true; font.pixelSize: 14
            horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
            wrapMode: Text.Wrap
        }
        background: Rectangle { implicitHeight: 40; radius: 8; color: root.acc; opacity: b.enabled ? (b.down ? 0.8 : 1) : 0.5 }
    }
    component Field: TextField {
        Layout.fillWidth: true
        color: root.fg
        selectByMouse: true
        placeholderTextColor: root.mut
        font.pixelSize: 14
        padding: 9
        background: Rectangle { radius: 8; color: root.field; border.color: root.line }
    }
    component Label2: Text { color: root.mut; font.pixelSize: 14; Layout.topMargin: 6 }
    component Muted: Text { color: root.mut; font.pixelSize: 14; wrapMode: Text.Wrap; Layout.fillWidth: true }

    // ── Layout ──────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Header
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: hdr.implicitHeight + 32
            color: root.bg
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: root.line }
            Flow {
                id: hdr
                anchors.fill: parent
                anchors.margins: 16
                anchors.leftMargin: 20
                spacing: 12
                Text {
                    textFormat: Text.StyledText
                    text: "<font color='#f4b728'>ⓩ</font> Offline Zcash <font color='#8a93a6'>"
                          + (root.st && root.st.version ? "v" + root.st.version : "") + "</font>"
                    color: root.fg; font.pixelSize: 18; font.bold: true
                }
                Pill {
                    text: root.reachError !== "" ? "module not reachable: " + root.reachError
                          : root.st ? (root.st.name || "not joined") + (root.st.role ? " · " + root.st.role : "") : "…"
                    textColor: root.reachError !== "" ? root.bad : root.mut
                }
                Pill {
                    visible: root.st !== null
                    text: !root.running ? "stopped" : root.st && root.st.online ? "gateway / online" : "offline"
                    textColor: root.st && root.st.online ? root.ok : root.mut
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

                // ── Setup (not joined / not running) ──
                Card {
                    id: setup
                    visible: root.showSetup
                    title: root.joined ? "Start node" : "Join the mesh"
                    Layout.fillWidth: true
                    Layout.maximumWidth: 720
                    Layout.leftMargin: 20; Layout.rightMargin: 20
                    property string msg: ""
                    property bool busy: false

                    Muted {
                        text: root.joined
                              ? "This device has joined the fleet as " + (root.st ? root.st.name + " (" + root.st.role + ")" : "") + ". Start the node to join the mesh."
                              : "Start without an invite to create your keys locally and find a gateway. Optional: paste an organiser invite instead."
                    }
                    Label2 { visible: !root.joined; text: "Invite JSON" }
                    Rectangle {
                        visible: !root.joined
                        Layout.fillWidth: true
                        implicitHeight: 140
                        radius: 8; color: root.field; border.color: root.line
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
                                placeholderText: "{\"fleet\": …, \"name\": \"A\", \"role\": \"payer\", …}"
                                placeholderTextColor: root.mut
                                background: null
                            }
                        }
                    }
                    Label2 { text: "Dial address (optional)" }
                    Field { id: dial; objectName: "dialInput"; placeholderText: "host:port of mesh neighbours, comma-separated, e.g. 192.168.1.20:7790" }
                    Label2 { text: "Listen address (optional)" }
                    Field { id: listenAddr; objectName: "listenInput"; placeholderText: "mesh listen host:port, default 0.0.0.0:7777" }
                    Switch {
                        id: bluetooth
                        objectName: "bluetoothToggle"
                        text: "Bluetooth LE (Linux only, experimental)"
                        palette.windowText: root.fg
                        palette.text: root.fg
                        checked: false
                        Accessible.name: text
                    }
                    Muted { text: "Requires Bluetooth enabled and a BlueZ adapter with advertising support. Real-radio testing is still pending." }
                    Btn {
                        objectName: "startNodeButton"
                        text: setup.busy ? "Working…" : (root.joined ? "Start node" : invite.text.trim() === "" ? "Start without invite" : "Join and start")
                        enabled: !setup.busy
                        onClicked: {
                            setup.busy = true
                            var peers = dial.text.split(/[\s,]+/).filter(function (s) { return s !== "" })
                            var opts = peers.length ? { dial: peers } : {}
                            if (listenAddr.text.trim() !== "") opts.listen = listenAddr.text.trim()
                            opts.ble = bluetooth.checked
                            var startNode = function () {
                                setup.msg = "Starting…"
                                root.call("start", [JSON.stringify(opts)], function (r) {
                                    setup.busy = false
                                    setup.msg = r.error ? "Start failed: " + r.error : "Started"
                                    root.refresh()
                                }, 60000)
                            }
                            if (root.joined || invite.text.trim() === "") { startNode(); return }
                            setup.msg = "Joining…"
                            root.call("join", [invite.text.trim()], function (r) {
                                if (r.error) { setup.busy = false; setup.msg = "Join failed: " + r.error; return }
                                setup.msg = r.message || "Joined"
                                startNode()
                            })
                        }
                    }
                    Muted { text: setup.msg; visible: text !== "" }
                }

                // ── Not reachable ──
                Card {
                    visible: root.st === null
                    title: "Offline Zcash"
                    Layout.fillWidth: true
                    Layout.maximumWidth: 720
                    Layout.leftMargin: 20; Layout.rightMargin: 20
                    Muted {
                        text: root.reachError !== ""
                              ? "Waiting for zcash_offline_module: " + root.reachError + ". Load the module in Basecamp's Modules view; this page retries every few seconds."
                              : "Connecting to zcash_offline_module…"
                    }
                }

                // ── Main grid ──
                GridLayout {
                    visible: root.running
                    Layout.fillWidth: true
                    Layout.leftMargin: 20; Layout.rightMargin: 20
                    Layout.maximumWidth: 1200
                    columns: Math.max(1, Math.min(3, Math.floor((scroll.availableWidth - 40) / 336)))
                    columnSpacing: 16; rowSpacing: 16

                    // Wallet
                    Card {
                        id: wallet
                        title: "Wallet"
                        Layout.fillWidth: true; Layout.fillHeight: true; Layout.preferredWidth: 320
                        property string msg: ""
                        property bool busy: false
                        Text {
                            objectName: "balance"
                            text: root.st && root.st.wallet ? root.zec(root.st.wallet.spendable) : "…"
                            color: root.fg; font.pixelSize: 26; font.weight: Font.DemiBold
                        }
                        Muted {
                            text: root.st && root.st.wallet ? "locked in pending payments: " + root.zec(root.st.wallet.locked) + " · "
                                            + root.st.wallet.notes + " notes · synced to height " + root.st.wallet.tip : ""
                        }
                        Muted {
                            text: root.st && root.st.selected_gateway
                                  ? "Gateway " + root.st.selected_gateway.name + " · " + root.st.selected_gateway.peer_id + " · " + root.st.selected_gateway.hops + " hops · tip " + root.st.selected_gateway.tip + " (gateway-reported)"
                                  : root.st && root.st.online ? "Local chain RPC · results are gateway-reported" : "Finding a gateway · sync and results are gateway-reported"
                        }
                        Muted { text: root.st ? root.st.gateway_warning || "" : ""; color: root.warn }
                        Btn {
                            text: root.st && root.st.online ? "Sync now (from the node)" : "Sync now (from a gateway over the mesh)"
                            enabled: !wallet.busy
                            onClicked: {
                                wallet.busy = true; wallet.msg = "Syncing…"
                                root.call("sync_now", [], function (r) {
                                    wallet.busy = false
                                    wallet.msg = r.error ? r.error
                                        : r.requested_from ? "Asked " + r.requested_from + " for blocks; balance updates when they arrive"
                                        : "Synced to " + r.tip + (r.new_notes !== undefined ? ", " + r.new_notes + " new notes" : "")
                                    root.refresh()
                                }, 120000)
                            }
                        }
                        Btn {
                            visible: root.st !== null && !root.st.online
                            text: "Get test funds (5 × 0.1 ZEC)"
                            enabled: !wallet.busy
                            onClicked: {
                                wallet.busy = true; wallet.msg = "Asking the gateway for test funds…"
                                root.call("request_funds", [5, "0.1"], function (r) {
                                    wallet.busy = false
                                    wallet.msg = r.error ? r.error
                                        : "Asked " + r.requested_from + "; funds arrive after they are mined (about a minute)"
                                })
                            }
                        }
                        Muted { text: wallet.msg; visible: text !== "" }
                        Item { Layout.fillHeight: true }
                    }

                    // Pay offline
                    Card {
                        id: pay
                        title: "Pay offline"
                        Layout.fillWidth: true; Layout.fillHeight: true; Layout.preferredWidth: 320
                        property string msg: ""
                        property bool busy: false
                        Label2 { text: "Recipient Zcash address" }
                        Field {
                            id: to
                            objectName: "recipientAddress"
                            placeholderText: "uregtest…"
                            Accessible.name: "Recipient Zcash address"
                        }
                        Label2 { text: "Amount (ZEC)" }
                        Field { id: amt; objectName: "amountInput"; text: "0.01"; inputMethodHints: Qt.ImhFormattedNumbersOnly }
                        Btn {
                            text: "Max"
                            enabled: !pay.busy
                            onClicked: root.call("max_payment", [], function (r) {
                                if (r.error) { pay.msg = r.error; return }
                                amt.text = r.amount
                                pay.msg = "Builder fee: " + root.zec(r.fee_zat) + ". Quote updates if your spendable notes change."
                            })
                        }
                        Btn {
                            text: pay.busy ? "Proving…" : "Build, prove and send over the mesh"
                            enabled: !pay.busy && to.text.trim() !== ""
                            onClicked: {
                                pay.busy = true; pay.msg = "Proving…"
                                root.call("pay", [to.text.trim(), amt.text.trim()], function (r) {
                                    pay.busy = false
                                    pay.msg = r.error ? r.error : "Sent " + String(r.txid || "").slice(0, 16) + "… (" + r.state + ")"
                                    root.refresh()
                                }, 180000)
                            }
                        }
                        Muted { objectName: "payResult"; text: pay.msg; visible: text !== "" }
                        Muted { text: "Use the address shared by the recipient. Unknown mesh address: submitted, no payee receipt. Mined is gateway-reported." }
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
                        }
                        Btn {
                            id: copyBtn
                            property bool copied: false
                            text: copied ? "Copied" : "Copy address"
                            enabled: addr.text !== ""
                            onClicked: { addr.selectAll(); addr.copy(); addr.deselect(); copied = true; copyReset.restart() }
                            Timer { id: copyReset; interval: 1500; onTriggered: copyBtn.copied = false }
                        }
                        Muted { text: "Open or unknown payers are not accepted pending. Wait for gateway-reported inclusion; it is not independent chain verification." }
                        Item { Layout.fillHeight: true }
                    }

                    // Payments
                    Card {
                        title: "Payments"
                        Layout.fillWidth: true
                        Layout.columnSpan: parent.columns
                        // Table header
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Repeater {
                                model: [["When", 80], ["Direction", 80], ["With", 80], ["Amount", 110], ["State", 170], ["Transaction", -1]]
                                Text {
                                    text: modelData[0]; color: root.mut; font.pixelSize: 12
                                    Layout.preferredWidth: modelData[1] > 0 ? modelData[1] : -1
                                    Layout.fillWidth: modelData[1] < 0
                                }
                            }
                        }
                        Repeater {
                            model: root.pays.payments
                            ColumnLayout {
                                id: prow
                                required property var modelData
                                readonly property bool open: root.openHistory[modelData.txid] === true
                                Layout.fillWidth: true
                                spacing: 4
                                Rectangle { Layout.fillWidth: true; height: 1; color: root.line }
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8
                                    Text { text: root.hms(prow.modelData.created_ms); color: root.fg; font.pixelSize: 14; Layout.preferredWidth: 80; Layout.alignment: Qt.AlignTop }
                                    Text { text: prow.modelData.direction; color: root.fg; font.pixelSize: 14; Layout.preferredWidth: 80; Layout.alignment: Qt.AlignTop }
                                    Text { text: prow.modelData.counterparty; textFormat: Text.PlainText; color: root.fg; font.pixelSize: 12; Layout.preferredWidth: 150; Layout.alignment: Qt.AlignTop; wrapMode: Text.WrapAnywhere }
                                    Text { text: root.zec(prow.modelData.amount); color: root.fg; font.pixelSize: 14; Layout.preferredWidth: 110; Layout.alignment: Qt.AlignTop }
                                    ColumnLayout {
                                        Layout.preferredWidth: 170; Layout.alignment: Qt.AlignTop
                                        spacing: 2
                                        Text { text: prow.modelData.state; color: root.stateColor(prow.modelData.state); font.pixelSize: 14; font.bold: true; wrapMode: Text.Wrap; Layout.fillWidth: true }
                                        Muted { visible: prow.modelData.direction === "sent" && !prow.modelData.receipt_peer; text: "No payee receipt"; font.pixelSize: 12 }
                                        Text {
                                            text: (prow.open ? "▾ " : "▸ ") + "history"
                                            color: root.mut; font.pixelSize: 13
                                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: {
                                                var o = Object.assign({}, root.openHistory)
                                                if (prow.open) delete o[prow.modelData.txid]; else o[prow.modelData.txid] = true
                                                root.openHistory = o
                                            } }
                                        }
                                    }
                                    TextEdit {
                                        text: prow.modelData.txid; readOnly: true; selectByMouse: true
                                        color: root.fg; font.family: root.mono; font.pixelSize: 12
                                        wrapMode: TextEdit.WrapAnywhere
                                        Layout.fillWidth: true; Layout.alignment: Qt.AlignTop
                                    }
                                }
                                Repeater {
                                    model: prow.open ? (prow.modelData.history || []) : []
                                    Text {
                                        text: root.hms(modelData.ms) + "  " + modelData.event
                                        color: root.mut; font.pixelSize: 12; wrapMode: Text.Wrap
                                        Layout.fillWidth: true; Layout.leftMargin: 384
                                    }
                                }
                            }
                        }
                        Rectangle { visible: root.pays.payments.length === 0; Layout.fillWidth: true; height: 1; color: root.line }
                        Muted { visible: root.pays.payments.length === 0; text: "No payments yet" }
                        Repeater {
                            model: root.pays.refusals
                            Text {
                                Layout.fillWidth: true; wrapMode: Text.Wrap
                                text: "Refused from " + modelData.from + ": code " + (modelData.reject ? modelData.reject.code + " " + modelData.reject.reason : "?")
                                      + (modelData.txid ? " (" + String(modelData.txid).slice(0, 16) + "…)" : "")
                                color: root.bad; font.pixelSize: 13
                            }
                        }
                    }

                    // Mesh
                    Card {
                        title: "Mesh"
                        Layout.fillWidth: true
                        Layout.columnSpan: parent.columns
                        Text {
                            Layout.fillWidth: true; wrapMode: Text.WrapAnywhere
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
                                  ? (root.st.ble_running ? "on" : "stopped — restart node") : "off")
                        }
                    }
                }
                Item { implicitHeight: 12 }
            }
        }
    }
}
