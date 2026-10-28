import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    width: 1100; height: 760
    color: bg
    readonly property string moduleName: "zcash_offline_module"
    // Palette shared with the Offline Zcash wallet, Organiser and web UI.
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
    property var st: null
    property var submissions: []
    property bool loaded: false
    property string message: ""
    property bool failed: false
    property bool busy: false
    property bool refreshing: false
    property bool settingsLoaded: false
    property string reachError: ""   // last failed status call; the badge must not show a stale state
    readonly property bool joined: st !== null && st.joined === true
    readonly property bool allowed: joined && st.role === "gateway"
    readonly property bool testnet: st !== null && st.network === "testnet"
    readonly property bool online: st !== null && reachError === "" && st.running === true && st.online === true
    readonly property bool syncing: st !== null && st.syncing === true
    readonly property bool narrow: width < 640
    property string servers: ""   // testnet: the fleet's TLS lightwalletd servers
    // Regtest gateways need the plain http:// Zakura JSON-RPC; the engine gives the full reason on check.
    readonly property bool rpcInvalid: !testnet && !/^http:\/\//i.test(rpc.text.trim())
    readonly property string chainUrl: testnet ? servers : rpc.text.trim()

    function unwrap(payload) {
        var v = payload
        for (var i = 0; i < 3 && typeof v === "string"; i++) {
            try { v = JSON.parse(v) } catch (e) { return { error: String(v) } }
        }
        if (v && typeof v === "object" && v.success !== undefined && v.value !== undefined && v.error !== undefined)
            return v.success ? unwrap(v.value) : { error: String(v.error) }
        if (v && typeof v === "object" && v.error !== undefined && v.message) return { error: String(v.error) + " (" + (v.method || "call") + "): " + v.message }   // LogosQmlBridge CallError detail
        return v === null || v === undefined ? { error: "empty reply" } : v
    }
    function call(method, args, cb, timeout) {
        logos.callModuleAsync(moduleName, method, args, function (v) { cb(unwrap(v)) }, timeout || 30000)
    }
    function zec(z) { return (Number(z || 0) / 1e8).toFixed(8).replace(/0+$/, "").replace(/\.$/, "") + " ZEC" }
    // A gateway result is this gateway's own chain-service report: [text, colour].
    function resultInfo(s) {
        if (!s) return ["Unknown", mut]
        if (typeof s === "string") return s === "Mempool" ? ["In mempool", warn] : s === "Submitted" ? ["Submitted, not included yet", warn]
                                         : s === "Expired" ? ["Expired", bad] : [s, warn]
        if (s.Mined) return ["Mined at height " + s.Mined.height + " · gateway-reported", ok]
        if (s.Included) return ["Included at height " + s.Included.height + " · gateway-reported, fewer than 10 confirmations", info]
        if (s.Retryable) return ["Retrying: " + s.Retryable.reason, warn]
        if (s.Conflict) return ["Conflict: " + s.Conflict.reason, bad]
        if (s.Rejected) return ["Rejected: " + s.Rejected.reason, bad]
        return [JSON.stringify(s), warn]
    }
    function refresh() {
        if (refreshing) return
        refreshing = true
        call("status", [], function (s) {
            if (s.error) { root.refreshing = false; root.reachError = s.error; return }
            root.reachError = ""
            root.st = s
            if (!root.settingsLoaded && s.joined) {
                if (s.rpc && s.network !== "testnet") rpc.text = s.rpc
                mine.checked = !!s.mine
                root.settingsLoaded = true
                if (s.network === "testnet") root.call("fleet", [], function (f) { if (!f.error) root.servers = (f.servers || []).join(",") })
            }
            if (!s.running) { root.refreshing = false; root.loaded = true; return }
            root.call("list_payments", [], function (p) {
                root.refreshing = false; root.loaded = true
                if (!p.error) root.submissions = p.gateway || []
            })
        })
    }
    function checkRpc(startAfter) {
        root.busy = true; root.failed = false; root.message = root.testnet ? "Checking TLS lightwalletd…" : "Checking getblockchaininfo…"
        root.call("check_rpc", [root.chainUrl], function (info) {
            if (info.error) { root.busy = false; root.failed = true; root.message = "Chain service check failed: " + info.error + (root.testnet ? ". Check your internet connection." : "."); return }
            root.message = "Chain service answers · " + info.chain + " · height " + info.blocks
            if (!startAfter) { root.busy = false; return }
            var opts = { rpc: root.chainUrl, mine: mine.checked && !root.testnet }
            if (listenAddr.text.trim() !== "") opts.listen = listenAddr.text.trim()
            var peers = dial.text.split(/[\s,]+/).filter(function (s) { return s !== "" })
            if (peers.length) opts.dial = peers
            root.call("start", [JSON.stringify(opts)], function (s) {
                root.busy = false; root.failed = !!s.error
                root.message = s.error ? "Gateway not started: " + s.error : "Gateway running on the shared device · " + s.name + ". Its wallet syncs in the background."
                root.refresh()
                if (!s.error) root.call("sync_now", [], function (r) {
                    if (r.error) { root.failed = true; root.message = "Gateway started; wallet sync failed: " + r.error }
                    root.refresh()
                })
            }, 60000)
        }, 70000)
    }
    Component.onCompleted: { logos.onModuleEvent(moduleName, "node_event"); refresh() }
    Connections {
        target: logos
        function onModuleEventReceived(module, eventName, data) { if (module === root.moduleName) root.refresh() }
    }
    Timer { interval: 3000; repeat: true; running: true; onTriggered: root.refresh() }
    function leave() {
        root.busy = true; root.failed = false; root.message = "Leaving…"
        root.call("leave", [], function (r) {
            root.busy = false; root.failed = !!r.error
            root.message = r.error ? "Could not leave: " + r.error : r.message
            if (!r.error) { root.settingsLoaded = false; root.servers = ""; root.submissions = []; rpc.text = "http://127.0.0.1:18232"; mine.checked = false }
            root.refresh()
        }, 60000)
    }
    Popup {
        id: leaveConfirm
        parent: Overlay.overlay; anchors.centerIn: parent
        modal: true; focus: true; padding: 20
        width: Math.min(root.width - 40, 480)
        background: Rectangle { color: root.card; border.color: root.line; radius: 10 }
        contentItem: ColumnLayout {
            spacing: 10
            Text { text: "Leave this session?"; color: root.fg; font.pixelSize: 18; font.bold: true }
            Label2 { text: "The gateway and node stop. This device's keys, wallet and submission history move to a backup folder next to it; nothing is deleted. Exported session folders stay where they are. Afterwards you can join another invite, for example a testnet session." }
            RowLayout { Layout.fillWidth: true; spacing: 10
                Btn { secondary: true; text: "Cancel"; Layout.fillWidth: true; onClicked: leaveConfirm.close() }
                Btn { objectName: "confirmLeaveButton"; text: "Leave and keep a backup"; Layout.fillWidth: true; onClicked: { leaveConfirm.close(); root.leave() } } }
        }
    }

    component Card: Rectangle {
        id: cardRoot
        default property alias content: col.data
        property string title: ""
        Layout.fillWidth: true
        color: root.card; border.color: root.line; radius: 10
        implicitHeight: col.implicitHeight + 32
        Accessible.role: Accessible.Grouping
        Accessible.name: title
        ColumnLayout {
            id: col
            anchors.fill: parent; anchors.margins: 16
            spacing: 6
            Text { text: cardRoot.title.toUpperCase(); color: root.mut; font.pixelSize: 13; font.letterSpacing: 0.8; Layout.bottomMargin: 4 }
        }
    }
    component Label2: Text { color: root.mut; font.pixelSize: 14; wrapMode: Text.Wrap; Layout.fillWidth: true; textFormat: Text.PlainText }
    component Field: TextField {
        id: f
        property bool invalid: false
        Layout.fillWidth: true; color: root.fg; font.pixelSize: 14; padding: 10; selectByMouse: true
        placeholderTextColor: "#7d8699"
        background: Rectangle { color: root.field; radius: 8; border.width: f.activeFocus ? 2 : 1; border.color: f.invalid ? root.bad : f.activeFocus ? root.acc : root.line }
    }
    component Btn: Button {
        id: b
        property bool secondary: false
        padding: 10; hoverEnabled: true
        Accessible.name: text
        Keys.onReturnPressed: if (enabled) clicked()
        contentItem: Text { text: b.text; color: b.secondary ? root.fg : "#111"; font.bold: true; font.pixelSize: 14; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter; wrapMode: Text.Wrap }
        background: Rectangle {
            implicitHeight: 40; radius: 8
            color: b.secondary ? (b.hovered && b.enabled ? "#222733" : "transparent") : root.acc
            opacity: b.enabled ? (b.down ? 0.8 : 1) : 0.45
            border.width: b.visualFocus ? 2 : 1
            border.color: b.visualFocus ? root.fg : b.secondary ? root.line : "transparent"
        }
    }
    component Note: Rectangle {
        property alias text: nt.text
        property color tone: root.warn
        Layout.fillWidth: true
        visible: nt.text !== ""
        radius: 8; color: Qt.rgba(tone.r, tone.g, tone.b, 0.08); border.color: Qt.rgba(tone.r, tone.g, tone.b, 0.45)
        implicitHeight: nt.implicitHeight + 16
        Accessible.role: Accessible.AlertMessage
        Accessible.name: nt.text
        Text { id: nt; anchors.fill: parent; anchors.margins: 8; color: parent.tone; font.pixelSize: 14; wrapMode: Text.Wrap; textFormat: Text.PlainText }
    }
    component Pill: Rectangle {
        property alias text: t.text
        property color textColor: root.mut
        radius: height / 2; color: root.card; border.color: root.line
        implicitHeight: t.implicitHeight + 6; implicitWidth: t.implicitWidth + 18
        Text { id: t; anchors.centerIn: parent; color: parent.textColor; font.pixelSize: 12; textFormat: Text.PlainText }
    }

    ScrollView {
        id: scroll
        anchors.fill: parent; anchors.margins: root.narrow ? 12 : 24
        contentWidth: availableWidth; clip: true
        ColumnLayout {
            width: Math.min(scroll.availableWidth, 860); spacing: 14
            Flow {
                Layout.fillWidth: true; spacing: 10
                Rectangle { width: 24; height: 24; radius: 12; color: root.acc; Accessible.ignored: true
                    Text { anchors.centerIn: parent; text: "Z"; color: "#111"; font.pixelSize: 15; font.bold: true } }
                Text { text: "Offline Zcash Gateway"; color: root.fg; font.pixelSize: 22; font.bold: true }
                Pill { visible: root.joined; text: root.joined ? [root.st.name, root.st.role, root.st.network || "regtest"].filter(function (x) { return !!x }).join(" · ") : "" }
                Pill {
                    visible: root.st !== null || root.reachError !== ""
                    text: root.reachError !== "" ? "service not answering" : root.syncing ? "syncing · height " + (root.st.wallet ? root.st.wallet.tip : 0) + (root.st.chain_tip ? " of " + root.st.chain_tip : "")
                          : root.online ? "gateway online" : root.st && root.st.running ? "running · gateway not online" : "stopped"
                    textColor: root.reachError !== "" ? root.bad : root.online || root.syncing ? root.ok : root.mut
                }
            }
            Label2 { text: "Bridges mesh payments to the chain. On Regtest the operator runs ths start outside Basecamp; on testnet the gateway uses TLS lightwalletd servers. The fleet chooses the network." }
            Label2 { visible: root.st === null && root.message === "" && root.reachError === ""; text: "Connecting to zcash_offline_module…" }
            Note { tone: root.bad; text: root.reachError !== "" ? "The Offline Zcash service did not answer: " + root.reachError + ". This page retries every 3 seconds." : "" }

            Card {
                visible: root.st !== null && !root.joined
                title: "1 · Join as the gateway"
                Label2 { text: "Paste the gateway invite created in Offline Zcash Organiser. Only an invite marked as the gateway can run one; installing this app grants nothing." }
                Rectangle {
                    Layout.fillWidth: true; Layout.preferredHeight: 120
                    radius: 8; color: root.field; border.color: invite.activeFocus ? root.acc : root.line; border.width: invite.activeFocus ? 2 : 1
                    ScrollView {
                        anchors.fill: parent; anchors.margins: 1
                        TextArea {
                            id: invite; objectName: "gatewayInvite"
                            color: root.fg; wrapMode: TextEdit.WrapAnywhere; selectByMouse: true; font.family: "monospace"; font.pixelSize: 12
                            placeholderText: "Paste your gateway invite JSON"; placeholderTextColor: "#7d8699"
                            background: null
                            Accessible.name: "Gateway invite JSON"
                        }
                    }
                }
                Btn {
                    text: root.busy ? "Joining…" : "Join device"; enabled: !root.busy && invite.text.trim() !== ""
                    Layout.fillWidth: true
                    onClicked: {
                        root.busy = true
                        root.call("join", [invite.text.trim()], function (r) {
                            root.busy = false; root.failed = !!r.error; root.message = r.error ? "Invite not accepted: " + r.error : r.message; root.refresh()
                        })
                    }
                }
            }
            Note {
                visible: root.joined && !root.allowed
                tone: root.bad
                text: root.joined && !root.allowed ? "This device's invite does not make it the gateway, and installing this app does not change that. Ask the organiser for a gateway invite, then leave this session and join with it." : ""
            }

            Card {
                title: root.joined ? "Chain service" : "2 · Chain service"
                visible: root.st !== null
                Label2 { text: root.testnet ? "Testnet TLS lightwalletd servers (from the session)" : "Chain RPC URL (Regtest, from ths endpoints)" }
                Repeater {
                    model: root.testnet ? root.servers.split(",").filter(function (s) { return s !== "" }) : []
                    Text { text: modelData; color: root.fg; font.family: "monospace"; font.pixelSize: 13; textFormat: Text.PlainText; Layout.fillWidth: true; wrapMode: Text.WrapAnywhere }
                }
                Field { id: rpc; objectName: "rpcUrl"; visible: !root.testnet; invalid: root.rpcInvalid; text: "http://127.0.0.1:18232"; Accessible.name: "Chain RPC URL" }
                Note {
                    tone: root.bad
                    text: root.rpcInvalid ? "This device is on a Regtest session, so the gateway needs the plain http:// Zakura JSON-RPC URL from ths endpoints (usually http://127.0.0.1:18232). https:// servers such as testnet.zec.rocks are testnet lightwalletd; to use testnet, leave this session and join a testnet invite." : ""
                }
                Switch {
                    id: mine; objectName: "mineBlocks"; visible: !root.testnet
                    text: "Mine blocks after each submission (Regtest)"
                    palette.windowText: root.fg; palette.text: root.fg; Accessible.name: text
                }
                Label2 { font.pixelSize: 12; text: root.testnet ? "Testnet has no gateway faucet and no mining. The session's organiser chose these servers." : "Mining also lets wallets request test funds. Use only the local ths Regtest chain, never real ZEC." }
                GridLayout {
                    Layout.fillWidth: true
                    columns: root.narrow ? 1 : 2; columnSpacing: 10; rowSpacing: 4
                    ColumnLayout { Layout.fillWidth: true; spacing: 2
                        Label2 { text: "Dial address (optional)" }
                        Field { id: dial; placeholderText: "operator-ip:7777"; Accessible.name: "Dial address" } }
                    ColumnLayout { Layout.fillWidth: true; spacing: 2
                        Label2 { text: "Listen address (optional)" }
                        Field { id: listenAddr; placeholderText: "0.0.0.0:7777"; Accessible.name: "Listen address"; enabled: !root.st || !root.st.running } }
                }
                Label2 { font.pixelSize: 12; text: "Mesh dial and listen apply on first start. The chain service and mining can change later without restarting the shared engine." }
                Flow {
                    Layout.fillWidth: true; spacing: 10; Layout.topMargin: 6
                    Btn { objectName: "checkRpcButton"; secondary: true; text: "Check chain service"; enabled: !root.busy && root.chainUrl !== "" && !root.rpcInvalid; onClicked: root.checkRpc(false) }
                    Btn {
                        objectName: "startGatewayButton"
                        text: root.busy ? "Working…" : root.online ? "Apply gateway settings" : "Start gateway"
                        enabled: !root.busy && root.allowed && root.chainUrl !== "" && !root.rpcInvalid
                        onClicked: root.checkRpc(true)
                    }
                }
                Label2 { visible: root.joined && !root.allowed; font.pixelSize: 12; text: "Start is disabled: this device is not the session's gateway." }
                Note { text: root.message; tone: root.failed ? root.bad : root.busy ? root.mut : root.ok }
                Label2 {
                    font.pixelSize: 12
                    visible: root.st !== null && root.st.running === true
                    color: root.st && !root.st.single_source ? root.mut : root.warn
                    text: root.st ? (root.st.single_source ? "Single chain source: results are not cross-checked." : "Cross-checked against " + (root.st.chain_sources || []).length + " chain sources.")
                                    + (root.testnet ? " Included means 1 confirmation and mined 10." : "") + " Results are gateway-reported, not proof-of-work or MMR inclusion proofs." : ""
                }
            }

            Card {
                visible: root.allowed
                title: "Gateway wallet"
                Text { text: root.st && root.st.wallet ? root.zec(root.st.wallet.spendable) : "Not started"; color: root.fg; font.pixelSize: 24; font.weight: Font.DemiBold }
                Label2 { text: root.st && root.st.wallet ? (root.syncing ? "syncing in the background · height " + root.st.wallet.tip + (root.st.chain_tip ? " of " + root.st.chain_tip : "") : "synced to height " + root.st.wallet.tip)
                                                         + (root.testnet ? "" : " · used to serve test funds") : "Start the gateway to sync its wallet." }
                Label2 { visible: !!(root.st && !root.syncing && root.st.sync_error); color: root.bad; text: root.st && root.st.sync_error ? "Last sync failed: " + root.st.sync_error : "" }
                Label2 { text: "Funding address" }
                TextEdit {
                    text: root.st && root.st.address ? root.st.address : ""
                    color: root.fg; font.family: "monospace"; font.pixelSize: 12; readOnly: true; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere; Layout.fillWidth: true
                    textFormat: TextEdit.PlainText; Accessible.name: "Gateway funding address"
                }
                Label2 { text: root.testnet ? "Testnet faucet: faucet.testnet.valargroup.dev · 0.125 TAZ / 24 h (the service may fail). There is no gateway faucet on testnet." : "Fund it from the operator's terminal (test coins):" }
                Rectangle {
                    visible: !root.testnet
                    Layout.fillWidth: true; radius: 6; color: root.field; border.color: root.line
                    implicitHeight: cmd.implicitHeight + 16
                    TextEdit {
                        id: cmd
                        anchors.fill: parent; anchors.margins: 8
                        text: "ths faucet " + (root.st && root.st.address ? root.st.address : "<address>") + " --amount 2"
                        color: root.fg; font.family: "monospace"; font.pixelSize: 12; readOnly: true; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere
                        textFormat: TextEdit.PlainText; Accessible.name: "Faucet command"
                    }
                }
                Btn {
                    secondary: true
                    visible: root.online
                    enabled: !root.syncing
                    text: root.syncing ? "Syncing…" : "Refresh gateway balance"
                    onClicked: root.call("sync_now", [], function (r) { root.failed = !!r.error; root.message = r.error ? "Sync failed: " + r.error : "Wallet sync started in the background."; root.refresh() })
                }
            }

            Card {
                visible: root.allowed
                title: "Submissions and results"
                Label2 { text: "Results come from this gateway's chain service. Wallets show them as gateway-reported, not chain proofs." }
                Label2 { visible: root.submissions.length === 0; color: root.fg; text: root.loaded ? "No submissions yet. Payments that reach this gateway over the mesh appear here." : "Loading…" }
                Repeater {
                    model: root.submissions
                    ColumnLayout {
                        id: sub
                        required property var modelData
                        readonly property var info: root.resultInfo(modelData.result && modelData.result.body ? modelData.result.body.status : null)
                        Layout.fillWidth: true; spacing: 2
                        Rectangle { Layout.fillWidth: true; height: 1; color: root.line; Layout.bottomMargin: 4 }
                        Text { text: sub.info[0]; color: sub.info[1]; font.pixelSize: 14; font.bold: true; wrapMode: Text.Wrap; Layout.fillWidth: true; textFormat: Text.PlainText }
                        Text { text: sub.modelData.txid; color: root.fg; font.family: "monospace"; font.pixelSize: 12; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true; textFormat: Text.PlainText }
                        Label2 { font.pixelSize: 12; text: "submitted " + sub.modelData.submissions + (sub.modelData.submissions === 1 ? " time" : " times") }
                    }
                }
            }
            Card {
                visible: root.joined
                title: "Session"
                Label2 { text: "To switch network or use another invite, leave this session. A backup of this device's keys and wallet is kept." }
                Btn { objectName: "leaveSessionButton"; secondary: true; text: "Leave session…"; enabled: !root.busy && !root.syncing; onClicked: leaveConfirm.open() }
                Label2 { visible: root.syncing; font.pixelSize: 12; text: "Leave is available when the wallet sync finishes." }
            }
            Item { implicitHeight: 16 }
        }
    }
}
