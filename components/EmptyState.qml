import QtQuick
import qs.Commons

Rectangle {
  id: root
  required property color textColor
  required property color dimColor
  property string title: "No contacts yet"
  property string body: "Press n to add someone. Drop a vCard on this window to import a list."

  color: "transparent"

  Column {
    anchors.centerIn: parent
    width: Math.min(parent.width - Style.space(48), Style.space(420))
    spacing: Style.space(10)

    Text {
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      text: root.title
      textFormat: Text.PlainText
      color: root.textColor
      font.family: Style.font.family
      font.pixelSize: Style.font.body
      wrapMode: Text.Wrap
    }
    Text {
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      text: root.body
      textFormat: Text.PlainText
      color: root.dimColor
      font.family: Style.font.family
      font.pixelSize: Style.font.body
      wrapMode: Text.Wrap
    }
  }
}
