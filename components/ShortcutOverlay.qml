import QtQuick
import qs.Commons
import qs.Ui
import "../keys/Keymap.js" as Keymap

Rectangle {
  id: root
  required property color textColor
  required property color backgroundColor
  required property color dimColor
  property var bindings: ({})
  signal dismissed()

  color: Qt.rgba(backgroundColor.r, backgroundColor.g, backgroundColor.b, 0.96)
  radius: Style.cornerRadius

  readonly property var columns: Keymap.rows(bindings)

  MouseArea {
    anchors.fill: parent
    onClicked: root.dismissed()
  }

  Column {
    anchors.fill: parent
    anchors.margins: Style.space(24)
    spacing: Style.space(16)

    Text {
      text: "Keyboard"
      textFormat: Text.PlainText
      color: root.textColor
      font.family: Style.font.family
      font.pixelSize: Style.font.body
      font.bold: true
    }

    Flickable {
      width: parent.width
      height: parent.height - Style.space(40)
      clip: true
      contentWidth: width
      contentHeight: grid.implicitHeight
      boundsBehavior: Flickable.StopAtBounds
      flickableDirection: Flickable.VerticalFlick

      Column {
        id: grid
        width: parent.width
        spacing: Style.space(18)

        Repeater {
          model: root.columns
          delegate: Column {
            width: grid.width
            spacing: Style.space(8)

            Text {
              text: String(modelData.group || "")
              textFormat: Text.PlainText
              color: root.dimColor
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
            }

            Repeater {
              model: modelData.items
              delegate: Row {
                width: grid.width
                spacing: Style.space(16)

                Text {
                  width: Style.space(160)
                  text: String(modelData.keys || "")
                  textFormat: Text.PlainText
                  color: root.textColor
                  font.family: Style.font.family
                  font.pixelSize: Style.font.body
                  elide: Text.ElideRight
                }
                Text {
                  text: String(modelData.action || "")
                  textFormat: Text.PlainText
                  color: root.dimColor
                  font.family: Style.font.family
                  font.pixelSize: Style.font.body
                  elide: Text.ElideRight
                }
              }
            }
          }
        }
      }
    }
  }
}
