import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs

Rectangle {
    id: root
    width: 1100; height: 760
    color: bg
    readonly property string moduleName: "zcash_offline_module"
    // Palette shared with the Offline Zcash wallet, Gateway and web UI.
    readonly property color bg: "#0f1115"
    readonly property color card: "#181b22"
    readonly property color line: "#2a2f3a"
    readonly property color fg: "#e7e9ee"
    readonly property color mut: "#9aa3b5"
    readonly property color ok: "#3fb950"
    readonly property color warn: "#e3a72f"
    readonly property color bad: "#ff6b63"
    readonly property color acc: "#f4b728"
    readonly property color field: "#0c0e12"
    property var st: null
    property var exported: null
    property string message: ""
    property bool busy: false
    property bool failed: false
    property int revision: 0     // bumps when the device list changes, so validation re-evaluates
    readonly property bool narrow: width < 700

    // Module QString results are JSON-encoded again by the Logos bridge.
    function unwrap(payload) {
        var v = payload
        for (var i = 0; i < 3 && typeof v === "string"; i++) {
            try { v = JSON.parse(v) } catch (e) { return { error: String(v) } }
        }
        if (v && typeof v === "object" && v.success !== undefined && v.value !== undefined && v.error !== undefined)
            return v.success ? unwrap(v.value) : { error: String(v.error) }
        return v === null || v === undefined ? { error: "empty reply" } : v
    }
    function call(method, args, cb) {
        logos.callModuleAsync(moduleName, method, args, function (v) { cb(unwrap(v)) }, 30000)
    }
    function refresh() {
        call("status", [], function (s) {
            if (s.error) { root.message = "Cannot reach zcash_offline_module: " + s.error; root.failed = true; return }
            if (root.st === null && root.failed && root.message.indexOf("Cannot reach") === 0) { root.message = ""; root.failed = false }
            root.st = s
            if (folder.text === "") folder.text = s.home + "/sessions/" + Date.now()
        })
    }
    // First problem that blocks fleet creation, or "".
    function problem() {
        void revision
        var names = {}
        if (sessionName.text.trim() === "") return "Enter a session name."
        if (!/^\d+$/.test(birthday.text.trim())) return "Birthday must be a block height (0 for the network default)."
        for (var i = 0; i < devices.count; i++) {
            var d = devices.get(i), n = d.deviceName.trim()
            if (!/^[A-Za-z0-9_-]+$/.test(n)) return "Device " + (i + 1) + ": use letters, digits, - or _ in the name."
            if (names[n.toLowerCase()]) return "Two devices are named " + n + "."
            names[n.toLowerCase()] = true
            if (!d.payerRole && !d.payeeRole && !d.relayRole && !d.gatewayRole) return n + " needs at least one role."
        }
        if (folder.text.trim() === "") return "Choose a session folder."
        var j = joinName.text.trim()
        if (joinName.enabled && j !== "" && !names[j.toLowerCase()]) return "Join this device as: " + j + " is not in the device list."
        return ""
    }
    function gatewayCount() {
        void revision
        var g = 0
        for (var i = 0; i < devices.count; i++) if (devices.get(i).gatewayRole) g++
        return g
    }
    Component.onCompleted: { logos.onModuleEvent(moduleName, "node_event"); refresh() }
    Connections {
        target: logos
        function onModuleEventReceived(module, eventName, data) { if (module === root.moduleName) root.refresh() }
    }
    Timer { interval: 3000; repeat: true; running: true; onTriggered: root.refresh() }

    ListModel {
        id: devices
        ListElement { deviceName: "Operator"; payerRole: false; payeeRole: false; relayRole: false; gatewayRole: true }
        ListElement { deviceName: "Alice"; payerRole: true; payeeRole: true; relayRole: false; gatewayRole: false }
        ListElement { deviceName: "Bob"; payerRole: true; payeeRole: true; relayRole: false; gatewayRole: false }
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
        Layout.fillWidth: true
        color: root.fg; font.pixelSize: 14; selectByMouse: true; padding: 10
        placeholderTextColor: "#7d8699"
        background: Rectangle { color: root.field; radius: 8; border.width: f.activeFocus ? 2 : 1
                                border.color: f.invalid ? root.bad : f.activeFocus ? root.acc : root.line }
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
    component RoleBox: CheckBox {
        palette.windowText: root.fg; palette.text: root.fg; palette.base: root.field; palette.button: root.field
        Accessible.name: text
    }
    FolderDialog {
        id: chooser
        title: "Choose the parent folder for a new session"
        onAccepted: folder.text = decodeURIComponent(selectedFolder.toString().replace(/^file:\/\//, "")) + "/offline-zcash-" + Date.now()
    }
    ScrollView {
        id: scroll
        anchors.fill: parent; anchors.margins: root.narrow ? 12 : 24
        contentWidth: availableWidth; clip: true
        ColumnLayout {
            width: Math.min(scroll.availableWidth, 900)
            spacing: 14
            RowLayout { spacing: 10
                Rectangle { width: 24; height: 24; radius: 12; color: root.acc; Accessible.ignored: true
                    Text { anchors.centerIn: parent; text: "Z"; color: "#111"; font.pixelSize: 15; font.bold: true } }
                Text { text: "Offline Zcash Organiser"; color: root.fg; font.pixelSize: 22; font.bold: true } }
            Label2 { text: "Create a test-coin session and give each person only their own invite. All Offline Zcash apps on this computer share one device." }
            Label2 { text: root.st === null ? "Connecting to zcash_offline_module…" : root.st.joined ? "This device: " + root.st.name + " · " + root.st.role + " · " + (root.st.network || "regtest") : "This device has not joined a session yet." }
            Label2 { font.pixelSize: 12; text: "Invites are optional: wallets can also start without one. Use invites when you want named devices and a fixed gateway." }

            Card {
                title: "1 · Session"
                GridLayout {
                    Layout.fillWidth: true
                    columns: root.narrow ? 1 : 3; columnSpacing: 12; rowSpacing: 4
                    ColumnLayout { Layout.fillWidth: true; spacing: 2
                        Label2 { text: "Session name" }
                        Field { id: sessionName; objectName: "sessionName"; text: "Three laptops"; Accessible.name: "Session name"; invalid: text.trim() === "" } }
                    ColumnLayout { Layout.fillWidth: true; spacing: 2
                        Label2 { text: "Network" }
                        ComboBox {
                            id: network; model: ["regtest", "testnet"]; Accessible.name: "Fleet network"; Layout.fillWidth: true; Layout.preferredHeight: 40
                            palette.button: root.field; palette.buttonText: root.fg; palette.window: root.card; palette.text: root.fg
                            palette.base: root.field; palette.highlight: root.acc; palette.highlightedText: "#111"
                        } }
                    ColumnLayout { Layout.fillWidth: true; spacing: 2
                        Label2 { text: "Birthday height" }
                        Field { id: birthday; text: "0"; inputMethodHints: Qt.ImhDigitsOnly; Accessible.name: "Fleet birthday height"; invalid: !/^\d+$/.test(text.trim()) } }
                }
                Label2 {
                    font.pixelSize: 12
                    text: (network.currentText === "testnet" ? "Public testnet with TAZ test coins; the gateway uses TLS lightwalletd. " : "Local Regtest chain run with ths start. ")
                          + "Birthday is the first block wallets scan; 0 uses " + (network.currentText === "testnet" ? "4465026" : "1") + ". Set it before anyone receives funds."
                }
            }

            Card {
                title: "2 · Devices and roles"
                Label2 { font.pixelSize: 12; text: "Names use letters, digits, - or _. Combine roles as needed; exactly one device usually runs the gateway." }
                Repeater {
                    model: devices
                    Flow {
                        required property int index
                        required property string deviceName
                        required property bool payerRole
                        required property bool payeeRole
                        required property bool relayRole
                        required property bool gatewayRole
                        Layout.fillWidth: true
                        spacing: 8
                        Field {
                            width: root.narrow ? parent.width : 180
                            text: deviceName
                            invalid: !/^[A-Za-z0-9_-]+$/.test(text.trim())
                            Accessible.name: "Device " + (index + 1) + " name"
                            onTextEdited: { devices.setProperty(index, "deviceName", text); root.revision++ }
                        }
                        RoleBox { text: "payer"; checked: payerRole; Accessible.name: deviceName + " payer role"; onToggled: { devices.setProperty(index, "payerRole", checked); root.revision++ } }
                        RoleBox { text: "payee"; checked: payeeRole; Accessible.name: deviceName + " payee role"; onToggled: { devices.setProperty(index, "payeeRole", checked); root.revision++ } }
                        RoleBox { text: "relay"; checked: relayRole; Accessible.name: deviceName + " relay role"; onToggled: { devices.setProperty(index, "relayRole", checked); root.revision++ } }
                        RoleBox { text: "gateway"; checked: gatewayRole; Accessible.name: deviceName + " gateway role"; onToggled: { devices.setProperty(index, "gatewayRole", checked); root.revision++ } }
                        Btn { secondary: true; text: "Remove"; Accessible.name: "Remove " + deviceName; enabled: devices.count > 1; onClicked: { devices.remove(index); root.revision++ } }
                    }
                }
                Label2 { visible: root.gatewayCount() === 0; color: root.warn; text: "No device has the gateway role. Invited payments then depend on a gateway outside this session." }
                Btn {
                    secondary: true
                    text: "Add device"
                    onClicked: { devices.append({ deviceName: "Device" + (devices.count + 1), payerRole: true, payeeRole: true, relayRole: false, gatewayRole: false }); root.revision++ }
                }
            }

            Card {
                title: "3 · Export"
                Label2 { text: "New session folder · existing fleet files are never overwritten" }
                RowLayout {
                    Layout.fillWidth: true
                    Field { id: folder; objectName: "exportFolder"; Accessible.name: "Session folder"; onTextEdited: root.revision++ }
                    Btn { secondary: true; text: root.narrow ? "Choose…" : "Choose parent folder…"; onClicked: chooser.open() }
                }
                Label2 { text: "Join this device as (optional)" }
                Field {
                    id: joinName; objectName: "joinDevice"
                    text: "Operator"; enabled: !root.st || !root.st.joined
                    placeholderText: "Leave empty to export only"
                    Accessible.name: "Join this device as"
                    onTextEdited: root.revision++
                }
                Label2 { visible: !joinName.enabled; font.pixelSize: 12; text: "This device already joined a session, so it exports only." }
                Btn {
                    objectName: "createFleetButton"
                    Layout.fillWidth: true; Layout.topMargin: 6
                    text: root.busy ? "Creating…" : "Create session and invites"
                    enabled: !root.busy && root.st !== null && root.problem() === ""
                    onClicked: {
                        var list = []
                        for (var i = 0; i < devices.count; i++) {
                            var d = devices.get(i), roles = []
                            if (d.payerRole) roles.push("payer")
                            if (d.payeeRole) roles.push("payee")
                            if (d.relayRole) roles.push("relay")
                            if (d.gatewayRole) roles.push("gateway")
                            list.push(d.deviceName.trim() + ":" + roles.join("+"))
                        }
                        var opts = { out: folder.text.trim(), name: sessionName.text.trim(), devices: list, network: network.currentText, birthday: Number(birthday.text) }
                        if ((!root.st || !root.st.joined) && joinName.text.trim() !== "") opts.join_device = joinName.text.trim()
                        root.busy = true; root.message = "Creating…"; root.failed = false
                        root.call("fleet_init", [JSON.stringify(opts)], function (r) {
                            root.busy = false; root.failed = !!r.error
                            if (r.error) { root.message = "Session not created: " + r.error; return }
                            root.exported = r
                            root.message = r.joined || "Session created; this device did not join."
                            root.refresh()
                        })
                    }
                }
                Label2 { visible: root.st !== null && root.problem() !== ""; color: root.warn; text: root.problem() }
                Note { text: root.message; tone: root.failed ? root.bad : root.busy ? root.mut : root.ok }
            }

            Card {
                visible: root.exported !== null
                title: "Invites ready"
                Note { tone: root.warn; text: "Each invite contains that person's private test keys. Send it only to its owner and keep the organiser's copy safe." }
                TextEdit {
                    Layout.fillWidth: true; readOnly: true; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere; textFormat: TextEdit.PlainText
                    text: root.exported ? root.exported.folder : ""; color: root.fg; font.family: "monospace"; font.pixelSize: 13
                    Accessible.name: "Session folder path"
                }
                Btn {
                    secondary: true
                    text: "Open session folder"
                    onClicked: root.call("open_fleet_folder", [root.exported.folder], function (r) {
                        root.failed = !!r.error
                        root.message = r.error ? "Could not open the folder: " + r.error + ". Copy the path above instead." : r.message
                    })
                }
                Repeater {
                    model: root.exported ? root.exported.devices : []
                    ColumnLayout {
                        Layout.fillWidth: true; spacing: 0
                        Text { text: modelData.name + " · " + modelData.role; color: root.fg; font.pixelSize: 14; font.bold: true; textFormat: Text.PlainText }
                        Text { text: modelData.invite_path; color: root.mut; font.family: "monospace"; font.pixelSize: 12; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true; textFormat: Text.PlainText }
                    }
                }
                Label2 { text: "Next: send each <name>.invite.json to its owner (AirDrop or USB). The gateway device opens Offline Zcash Gateway; everyone else opens Offline Zcash and pastes their invite." }
            }
            Item { implicitHeight: 16 }
        }
    }
}
