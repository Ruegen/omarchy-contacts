import QtQuick
import qs.Commons

Item {
  id: root
  property string name: ""
  property bool hasPhoto: false
  property string photoB64: ""
  property int size: Style.space(32)
  property color foreground: Color.foreground
  property color background: Color.background
  property color accent: Color.accent

  width: size
  height: size

  readonly property string initials: {
    var parts = String(name || "").trim().split(/\s+/)
    var a = parts[0] ? parts[0].charAt(0) : ""
    var b = parts.length > 1 ? parts[parts.length - 1].charAt(0) : ""
    return (a + b).toUpperCase()
  }

  Rectangle {
    anchors.fill: parent
    radius: width / 2
    color: Qt.rgba(accent.r, accent.g, accent.b, 0.18)
    border.width: Style.normalBorderWidth
    border.color: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.12)
    clip: true

    Image {
      anchors.fill: parent
      visible: root.hasPhoto && root.photoB64.length > 0
      source: visible ? ("data:image/jpeg;base64," + root.photoB64) : ""
      fillMode: Image.PreserveAspectCrop
      asynchronous: true
      cache: false
    }

    Text {
      anchors.centerIn: parent
      visible: !(root.hasPhoto && root.photoB64.length > 0)
      text: root.initials.length ? root.initials : "?"
      textFormat: Text.PlainText
      color: root.foreground
      font.family: Style.font.family
      font.pixelSize: Math.max(10, Math.round(root.size * 0.38))
    }
  }
}
