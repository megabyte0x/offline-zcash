import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs

Rectangle {
    id: root
    width: 1100; height: 760
    color: "#0f1115"
    readonly property string moduleName: "zcash_offline_module"
    readonly property color fg: "#e7e9ee"
    readonly property color mut: "#8a93a6"
    property var st: null
    property var exported: null
    property string message: ""
    property bool busy: false
    property bool failed: false

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
            if (s.error) { root.message = s.error; root.failed = true; return }
            root.st = s
            if (folder.text === "") folder.text = s.home + "/sessions/" + Date.now()
        })
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
    component Label2: Text { color: root.mut; font.pixelSize: 14; wrapMode: Text.Wrap; Layout.fillWidth: true }
    component Field: TextField {
        Layout.fillWidth: true
        color: root.fg; font.pixelSize: 14; selectByMouse: true; padding: 10
        placeholderTextColor: root.mut
        background: Rectangle { color: "#0c0e12"; radius: 8; border.color: "#2a2f3a" }
    }
    component Btn: Button {
        id: b
        padding: 10; hoverEnabled: true
        contentItem: Text { text: b.text; color: "#111"; font.bold: true; font.pixelSize: 14; horizontalAlignment: Text.AlignHCenter }
        background: Rectangle { radius: 8; color: "#f4b728"; opacity: b.enabled ? (b.down ? 0.8 : 1) : 0.5 }
    }
    FolderDialog {
        id: chooser
        title: "Choose the parent folder for a new session"
        onAccepted: folder.text = decodeURIComponent(selectedFolder.toString().replace(/^file:\/\//, "")) + "/offline-zcash-" + Date.now()
    }
    ScrollView {
        id: scroll
        anchors.fill: parent; anchors.margins: 24
        contentWidth: availableWidth; clip: true
        ColumnLayout {
            width: scroll.availableWidth
            spacing: 12
            Text { text: "Offline Zcash Organiser"; color: root.fg; font.pixelSize: 24; font.bold: true }
            Label2 { text: "Create a Regtest session and give each person only their own invite. All installed Offline Zcash apps share one device." }
            Label2 { text: root.st && root.st.joined ? "This device: " + root.st.name + " · " + root.st.role : "This device has not joined a fleet" }
            Label2 { text: "Session name" }
            Field { id: sessionName; objectName: "sessionName"; text: "Three laptops"; Accessible.name: "Session name" }
            Label2 { text: "Devices · names use letters, digits, - or _ · combine roles as needed" }
            Repeater {
                model: devices
                RowLayout {
                    required property int index
                    required property string deviceName
                    required property bool payerRole
                    required property bool payeeRole
                    required property bool relayRole
                    required property bool gatewayRole
                    Layout.fillWidth: true
                    Field {
                        Layout.preferredWidth: 160
                        text: deviceName
                        Accessible.name: "Device " + (index + 1) + " name"
                        onTextEdited: devices.setProperty(index, "deviceName", text)
                    }
                    CheckBox { text: "payer"; checked: payerRole; palette.windowText: root.fg; onToggled: devices.setProperty(index, "payerRole", checked) }
                    CheckBox { text: "payee"; checked: payeeRole; palette.windowText: root.fg; onToggled: devices.setProperty(index, "payeeRole", checked) }
                    CheckBox { text: "relay"; checked: relayRole; palette.windowText: root.fg; onToggled: devices.setProperty(index, "relayRole", checked) }
                    CheckBox { text: "gateway"; checked: gatewayRole; palette.windowText: root.fg; onToggled: devices.setProperty(index, "gatewayRole", checked) }
                    Button { text: "Remove"; enabled: devices.count > 1; onClicked: devices.remove(index) }
                }
            }
            Button {
                text: "Add device"
                onClicked: devices.append({ deviceName: "Device" + (devices.count + 1), payerRole: true, payeeRole: true, relayRole: false, gatewayRole: false })
            }
            Label2 { text: "New session folder · existing fleet files will not be overwritten" }
            RowLayout {
                Layout.fillWidth: true
                Field { id: folder; objectName: "exportFolder"; Accessible.name: "Session folder" }
                Button { text: "Choose parent folder…"; onClicked: chooser.open() }
            }
            Label2 { text: "Join this device as (optional device name, leave empty to export only)" }
            Field {
                id: joinName; objectName: "joinDevice"
                text: "Operator"; enabled: !root.st || !root.st.joined
                Accessible.name: "Join this device as"
            }
            Btn {
                objectName: "createFleetButton"
                text: root.busy ? "Creating…" : "Create fleet and invites"
                enabled: !root.busy && root.st !== null && sessionName.text.trim() !== "" && folder.text.trim() !== ""
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
                    var opts = { out: folder.text.trim(), name: sessionName.text.trim(), devices: list }
                    if ((!root.st || !root.st.joined) && joinName.text.trim() !== "") opts.join_device = joinName.text.trim()
                    root.busy = true; root.message = "Creating…"; root.failed = false
                    root.call("fleet_init", [JSON.stringify(opts)], function (r) {
                        root.busy = false; root.failed = !!r.error
                        if (r.error) { root.message = r.error; return }
                        root.exported = r
                        root.message = r.joined || "Fleet created; this device did not join"
                        root.refresh()
                    })
                }
            }
            Text { text: root.message; color: root.failed ? "#f85149" : "#3fb950"; wrapMode: Text.Wrap; Layout.fillWidth: true; font.pixelSize: 14 }
            ColumnLayout {
                visible: root.exported !== null
                Layout.fillWidth: true
                Label2 { text: "Invites ready · private test keys inside. Keep the organiser's copy safe." }
                TextEdit {
                    Layout.fillWidth: true; readOnly: true; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere
                    text: root.exported ? root.exported.folder : ""; color: root.fg; font.pixelSize: 14
                }
                Button {
                    text: "Open session folder"
                    onClicked: Qt.openUrlExternally("file://" + encodeURIComponent(root.exported.folder).replace(/%2F/g, "/"))
                }
                Repeater {
                    model: root.exported ? root.exported.devices : []
                    Label2 { text: modelData.name + " · " + modelData.role + "\n" + modelData.invite_path }
                }
                Label2 { text: "AirDrop each <name>.invite.json to its owner. If you joined as the gateway, open Offline Zcash Gateway next; other roles use Offline Zcash." }
            }
            Item { implicitHeight: 16 }
        }
    }
}
