import QtQuick
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "omarchy-contacts"

  readonly property var service: bar && bar.shell ? bar.shell.serviceFor("omarchy-contacts") : null
  readonly property color foreground: bar ? bar.barForeground : Color.foreground
  readonly property bool showCount: !!(settings && settings.showBarCount === true)
  readonly property int count: service ? Number(service.contactCount) || 0 : 0

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function applyToService() {
    if (service && typeof service.applySettings === "function")
      service.applySettings(settings)
  }

  onSettingsChanged: applyToService()
  onServiceChanged: applyToService()
  Component.onCompleted: applyToService()

  function openWindow() {
    if (!bar || !bar.shell) return
    if (typeof bar.shell.toggle === "function") bar.shell.toggle("omarchy-contacts", "{}")
    else if (typeof bar.shell.summon === "function") bar.shell.summon("omarchy-contacts", "{}")
  }

  function close() {}
  function closeForPopoutSwitch() {}

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    fontFamily: "Font Awesome 7 Free Solid"
    text: root.showCount ? "\uf2b9  " + String(root.count) : "\uf2b9"
    tooltipText: root.plain("Contacts" + (root.showCount ? " · " + String(root.count) : ""))
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.LeftButton) root.openWindow()
    }
  }

  function plain(s) {
    return String(s == null ? "" : s).replace(/[&<>]/g, "").substring(0, 120)
  }
}
