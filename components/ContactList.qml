import QtQuick
import QtQuick.Controls
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
  property bool focused: false
  signal activated(int index)
  signal hovered(int index)

  color: "transparent"
  clip: true

  function ensureVisible() {
    if (cursor < 0 || list.count <= 0) return
    var item = list.itemAtIndex(cursor)
    if (item) {
      var top = item.y
      var bottom = item.y + item.height
      if (top >= list.contentY && bottom <= list.contentY + list.height)
        return
    }
    list.positionViewAtIndex(Math.max(0, Math.min(cursor, list.count - 1)), ListView.Contain)
  }

  onCursorChanged: ensureVisible()

  ListView {
    id: list
    anchors.fill: parent
    anchors.rightMargin: Style.space(4)
    model: root.contacts
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    highlightFollowsCurrentItem: false
    keyNavigationEnabled: false
    currentIndex: -1
    spacing: Style.space(2)
    ScrollBar.vertical: ScrollBar {
      policy: list.contentHeight > list.height ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
    }
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
          photoFile: String(modelData.photo_file || "")
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
        onClicked: root.activated(index)
        onDoubleClicked: root.activated(index)
      }
    }
  }
}
