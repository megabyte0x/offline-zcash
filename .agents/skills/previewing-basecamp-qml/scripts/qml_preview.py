"""Render an Offline Zcash Basecamp QML app with a mock `logos` bridge and take screenshots.

Usage: python qml_preview.py SCENARIO.json OUT_DIR

SCENARIO keys:
  qml        path to the app's Main.qml (relative to the repo root or absolute)
  size       [width, height], default [1100, 900]
  replies    {method: reply}; a reply is a JSON value, or a list consumed one per call
             (the last element repeats). "status" is polled every 3 s by every app.
  steps      list of actions, run in order:
             {"click": objectName} | {"set": [objectName, property, value]}
             {"wait": ms} | {"shot": "name"} | {"scroll": pixels}
The script prints every module call it received, so the flow can be asserted from its output.
"""
import json, sys, pathlib
from PySide6.QtCore import QObject, Slot, Signal, QUrl, QTimer, QPointF, Qt
from PySide6.QtGui import QGuiApplication
from PySide6.QtQuick import QQuickView, QQuickItem
from PySide6.QtQml import QJSValue
from PySide6.QtTest import QTest

scenario = json.loads(pathlib.Path(sys.argv[1]).read_text())
out = pathlib.Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
repo = pathlib.Path(__file__).resolve().parents[4]


class Logos(QObject):
    moduleEventReceived = Signal(str, str, "QVariantList")

    def __init__(self, replies):
        super().__init__()
        self.replies = replies

    def reply(self, method):
        r = self.replies.get(method, {"error": f"mock has no reply for {method}"})
        if isinstance(r, list):
            r = r.pop(0) if len(r) > 1 else r[0]
        return r

    @Slot(str, str, "QVariant", QJSValue, int)
    def callModuleAsync(self, module, method, args, cb, timeout=0):
        if method != "status":
            print("call", method, json.dumps(args), flush=True)
        value = json.dumps(self.reply(method))  # the bridge delivers JSON strings
        QTimer.singleShot(50, lambda: cb.call([value]))

    @Slot(str, str)
    def onModuleEvent(self, module, event):
        pass


app = QGuiApplication(sys.argv)
view = QQuickView()
logos = Logos(scenario.get("replies", {}))
view.rootContext().setContextProperty("logos", logos)
w, h = scenario.get("size", [1100, 900])
view.setResizeMode(QQuickView.SizeRootObjectToView)
view.resize(w, h)
qml = pathlib.Path(scenario["qml"])
view.setSource(QUrl.fromLocalFile(str(qml if qml.is_absolute() else repo / qml)))
if view.status() != QQuickView.Ready:
    for e in view.errors():
        print("QML error:", e.toString())
    sys.exit(1)
view.show()


def find(name):
    item = view.rootObject().findChild(QQuickItem, name)
    if item is None:  # popups live in the overlay, outside the root item
        item = next((i for i in view.contentItem().findChildren(QQuickItem) if i.objectName() == name), None)
    if item is None:
        raise SystemExit(f"no item with objectName {name}")
    return item


steps = list(scenario.get("steps", []))


def run():
    if not steps:
        app.quit(); return
    s = steps.pop(0); delay = 300
    if "click" in s:
        item = find(s["click"])
        p = item.mapToScene(QPointF(item.width() / 2, item.height() / 2))
        QTest.mouseClick(view, Qt.LeftButton, Qt.NoModifier, p.toPoint())
    elif "set" in s:
        name, prop, value = s["set"]; find(name).setProperty(prop, value)
    elif "wait" in s:
        delay = s["wait"]
    elif "scroll" in s:
        flick = next(i for i in view.rootObject().findChildren(QQuickItem) if i.metaObject().className().startswith("QQuickFlickable"))
        flick.setProperty("contentY", s["scroll"])
    elif "shot" in s:
        path = out / f"{s['shot']}.png"; view.grabWindow().save(str(path)); print("shot", path, flush=True)
    QTimer.singleShot(delay, run)


QTimer.singleShot(1500, run)  # let the first status poll land
app.exec()
