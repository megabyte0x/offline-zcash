import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    width: 1100; height: 760
    color: "#0f1115"
    readonly property string moduleName: "zcash_offline_module"
    readonly property color fg: "#e7e9ee"
    readonly property color mut: "#8a93a6"
    property var st: null
    property var submissions: []
    property string message: ""
    property bool failed: false
    property bool busy: false
    property bool refreshing: false
    property bool settingsLoaded: false
    readonly property bool joined: st !== null && st.joined === true
    readonly property bool allowed: joined && String(st.role || "").split("+").indexOf("gateway") >= 0

    function unwrap(payload) {
        var v = payload
        for (var i = 0; i < 3 && typeof v === "string"; i++) {
            try { v = JSON.parse(v) } catch (e) { return { error: String(v) } }
        }
        if (v && typeof v === "object" && v.success !== undefined && v.value !== undefined && v.error !== undefined)
            return v.success ? unwrap(v.value) : { error: String(v.error) }
        return v === null || v === undefined ? { error: "empty reply" } : v
    }
    function call(method, args, cb, timeout) {
        logos.callModuleAsync(moduleName, method, args, function (v) { cb(unwrap(v)) }, timeout || 30000)
    }
    function refresh() {
        if (refreshing) return
        refreshing = true
        call("status", [], function (s) {
            if (s.error) { root.refreshing = false; root.failed = true; root.message = s.error; return }
            root.st = s
            if (!root.settingsLoaded) {
                if (s.rpc) rpc.text = s.rpc
                mine.checked = !!s.mine
                root.settingsLoaded = true
            }
            if (!s.running) { root.refreshing = false; return }
            root.call("list_payments", [], function (p) {
                root.refreshing = false
                if (!p.error) root.submissions = p.gateway || []
            })
        })
    }
    function checkRpc(startAfter) {
        root.busy = true; root.failed = false; root.message = "Checking getblockchaininfo…"
        root.call("check_rpc", [rpc.text.trim()], function (info) {
            if (info.error) { root.busy = false; root.failed = true; root.message = "RPC check failed: " + info.error; return }
            root.message = "RPC answers getblockchaininfo · " + info.chain + " · height " + info.blocks
            if (!startAfter) { root.busy = false; return }
            if (mine.checked && info.chain !== "test" && info.chain !== "regtest") {
                root.busy = false; root.failed = true; root.message = "Mining is for Regtest only. Disable mining for this chain."; return
            }
            var opts = { rpc: rpc.text.trim(), mine: mine.checked }
            if (listenAddr.text.trim() !== "") opts.listen = listenAddr.text.trim()
            var peers = dial.text.split(/[\s,]+/).filter(function (s) { return s !== "" })
            if (peers.length) opts.dial = peers
            root.call("start", [JSON.stringify(opts)], function (s) {
                root.busy = false; root.failed = !!s.error
                root.message = s.error ? s.error : "Gateway running on the shared device · " + s.name
                root.refresh()
                if (!s.error) root.call("sync_now", [], function (r) {
                    if (r.error) { root.failed = true; root.message = "Gateway started; wallet sync failed: " + r.error }
                    root.refresh()
                }, 120000)
            }, 60000)
        }, 70000)
    }
    Component.onCompleted: { logos.onModuleEvent(moduleName, "node_event"); refresh() }
    Connections {
        target: logos
        function onModuleEventReceived(module, eventName, data) { if (module === root.moduleName) root.refresh() }
    }
    Timer { interval: 3000; repeat: true; running: true; onTriggered: root.refresh() }

    component Label2: Text { color: root.mut; font.pixelSize: 14; wrapMode: Text.Wrap; Layout.fillWidth: true }
    component Field: TextField {
        Layout.fillWidth: true; color: root.fg; font.pixelSize: 14; padding: 10; selectByMouse: true
        placeholderTextColor: root.mut
        background: Rectangle { color: "#0c0e12"; radius: 8; border.color: "#2a2f3a" }
    }
    component Btn: Button {
        id: b
        padding: 10; hoverEnabled: true
        contentItem: Text { text: b.text; color: "#111"; font.bold: true; font.pixelSize: 14; horizontalAlignment: Text.AlignHCenter }
        background: Rectangle { radius: 8; color: "#f4b728"; opacity: b.enabled ? (b.down ? 0.8 : 1) : 0.5 }
    }
    ScrollView {
        id: scroll
        anchors.fill: parent; anchors.margins: 24
        contentWidth: availableWidth; clip: true
        ColumnLayout {
            width: scroll.availableWidth; spacing: 12
            Text { text: "Offline Zcash Gateway"; color: root.fg; font.pixelSize: 24; font.bold: true }
            Label2 { text: "Bridge mesh payments to the local chain. The operator runs ths start outside Basecamp." }
            Label2 { text: root.joined ? root.st.name + " · " + root.st.role + (root.st.running && root.st.online ? " · gateway online" : " · gateway not online") : "Join with a gateway invite, or create one in Offline Zcash Organiser." }
            ColumnLayout {
                visible: !root.joined
                Layout.fillWidth: true
                Label2 { text: "Gateway invite JSON" }
                ScrollView {
                    Layout.fillWidth: true; Layout.preferredHeight: 120
                    TextArea {
                        id: invite; objectName: "gatewayInvite"
                        color: root.fg; wrapMode: TextEdit.WrapAnywhere; selectByMouse: true
                        placeholderText: "Paste your invite JSON"; placeholderTextColor: root.mut
                        background: Rectangle { color: "#0c0e12"; border.color: "#2a2f3a"; radius: 8 }
                    }
                }
                Button {
                    text: "Join device"; enabled: !root.busy && invite.text.trim() !== ""
                    onClicked: {
                        root.busy = true
                        root.call("join", [invite.text.trim()], function (r) {
                            root.busy = false; root.failed = !!r.error; root.message = r.error || r.message; root.refresh()
                        })
                    }
                }
            }
            Text {
                visible: root.joined && !root.allowed
                text: "This invite does not grant the gateway role. Installing this app does not change your role. Ask the organiser for a gateway invite on a new device."
                color: "#f85149"; font.pixelSize: 16; wrapMode: Text.Wrap; Layout.fillWidth: true
            }
            Label2 { text: "Chain RPC URL" }
            Field { id: rpc; objectName: "rpcUrl"; text: "http://127.0.0.1:18232"; Accessible.name: "Chain RPC URL" }
            Switch { id: mine; objectName: "mineBlocks"; text: "Mine blocks (Regtest)"; palette.windowText: root.fg; Accessible.name: text }
            Label2 { text: "Mining also enables test-fund requests. Use only the local ths Regtest chain, never real ZEC." }
            RowLayout {
                Layout.fillWidth: true
                Field { id: dial; placeholderText: "Dial address (optional): operator-ip:7777"; Accessible.name: "Dial address" }
                Field { id: listenAddr; placeholderText: "Listen (optional): 0.0.0.0:7777"; Accessible.name: "Listen address"; enabled: !root.st || !root.st.running }
            }
            Label2 { text: "Mesh listen/dial options apply on first start; RPC and mining can be updated without replacing the shared engine." }
            RowLayout {
                Button { text: "Check RPC"; enabled: !root.busy; onClicked: root.checkRpc(false) }
                Btn {
                    objectName: "startGatewayButton"
                    text: root.busy ? "Working…" : root.st && root.st.online ? "Apply gateway settings" : "Start gateway"
                    enabled: !root.busy && root.allowed
                    onClicked: root.checkRpc(true)
                }
            }
            Text { text: root.message; color: root.failed ? "#f85149" : "#3fb950"; wrapMode: Text.Wrap; Layout.fillWidth: true; font.pixelSize: 14 }
            Label2 { visible: root.allowed; text: "Gateway wallet · " + (root.st && root.st.wallet ? (Number(root.st.wallet.spendable) / 1e8).toFixed(8) + " ZEC · synced to " + root.st.wallet.tip : "start to sync") }
            TextEdit {
                visible: root.allowed
                text: root.st && root.st.address ? root.st.address : ""
                color: root.fg; font.pixelSize: 13; readOnly: true; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere; Layout.fillWidth: true
            }
            Label2 { visible: root.allowed; text: "Fund this address from the operator's terminal (test coins):" }
            TextEdit {
                visible: root.allowed
                text: "ths faucet " + (root.st && root.st.address ? root.st.address : "<address>") + " --amount 2"
                color: root.fg; font.family: "monospace"; font.pixelSize: 13; readOnly: true; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere; Layout.fillWidth: true
            }
            Button {
                visible: root.allowed && !!root.st.online
                text: "Refresh gateway balance"
                onClicked: root.call("sync_now", [], function (r) { root.failed = !!r.error; root.message = r.error || "Wallet synced"; root.refresh() }, 120000)
            }
            Text { text: "Submissions and results"; color: root.fg; font.pixelSize: 18; font.bold: true }
            Label2 { visible: root.submissions.length === 0; text: "No submissions yet. Results are gateway-reported, not chain proofs." }
            Repeater {
                model: root.submissions
                Label2 {
                    readonly property var result: modelData.result.body.status
                    text: modelData.txid + "\n" + (result.Mined ? "mined at height " + result.Mined.height
                          : result.Rejected ? "rejected: " + result.Rejected.reason : "in mempool") + " · submissions " + modelData.submissions
                    color: result.Mined ? "#3fb950" : result.Rejected ? "#f85149" : root.fg
                }
            }
            Item { implicitHeight: 16 }
        }
    }
}
