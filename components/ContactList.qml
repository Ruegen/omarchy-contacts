import QtQuick
import qs.Commons

Rectangle {
  id: root
  property var contacts: []
  property int cursor: 0
  property string selectedUid: ""
  property color textColor: Color.foreground
  property color dimColor: Color.muted
  property color accent: Color.accent
  property color background: Color.background
  signal activated(string uid)
  signal hovered(int index)

  color: "transparent"
  clip: true

  function ensureVisible() {
    if (cursor < 0 || !list.itemAtIndex) return
    list.positionViewAtIndex(Math.max(0, cursor), ListView.Contain)
  }

  onCursorChanged: ensureVisible()

  ListView {
    id: list
    anchors.fill: parent
    model: root.contacts
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    currentIndex: root.cursor
    spacing: Style.space(2)
    delegate: Rectangle {
      required property var modelData
      required property int index
      width: list.width
      height: Style.space(52)
      radius: Math.min(Style.cornerRadius, 10)
      color: index === root.cursor
        ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.16)
        : "transparent"

      Row {
        anchors.fill: parent
        anchors.margins: Style.space(8)
        spacing: Style.space(10)

        Avatar {
          anchors.verticalCenter: parent.verticalCenter
          name: String(modelData.fn || "")
          hasPhoto: !!modelData.has_photo
          size: Style.space(32)
          foreground: root.textColor
          background: root.background
          accent: root.accent
        }

        Column {
          anchors.verticalCenter: parent.verticalCenter
          width: parent.width - Style.space(50)
          spacing: Style.space(2)

          Text {
            width: parent.width
            text: String(modelData.fn || "Unnamed")
            textFormat: Text.PlainText
            color: root.textColor
            elide: Text.ElideRight
            font.family: Style.font.family
            font.pixelSize: Style.font.body
          }
          Text {
            width: parent.width
            text: String(modelData.phone || modelData.email || modelData.org || "")
            textFormat: Text.PlainText
            color: root.dimColor
            elide: Text.ElideRight
            font.family: Style.font.family
            font.pixelSize: Style.font.body
          }
        }
      }

      MouseArea {
        anchors.fill: parent
        onClicked: {
          root.cursor = index
          root.activated(String(modelData.uid || ""))
        }
        onDoubleClicked: root.activated(String(modelData.uid || ""))
      }
    }
  }
}
