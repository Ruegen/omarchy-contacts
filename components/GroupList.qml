import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui

Rectangle {
  id: root
  property var groups: []
  property int cursor: 0
  property bool focused: false
  property bool naming: false
  property bool myCardActive: false
  property color textColor: Color.foreground
  property color dimColor: Color.muted
  property color accent: Color.accent
  property color background: Color.background
  signal activated(int index)
  signal myCardRequested()
  signal nameAccepted()

  color: "transparent"
  clip: true

  function takeNameFocus() {
    if (!nameField) return
    nameField.text = ""
    nameField.forceActiveFocus()
  }

  function nameText() {
    return nameField ? String(nameField.text || "") : ""
  }

  function ensureVisible() {
    if (cursor < 0 || !list.itemAtIndex) return
    list.positionViewAtIndex(Math.max(0, cursor), ListView.Contain)
  }

  onCursorChanged: ensureVisible()

  Rectangle {
    visible: root.focused
    anchors.left: parent.left
    width: 2
    height: parent.height
    color: root.accent
  }

  Column {
    anchors.fill: parent
    anchors.leftMargin: root.focused ? Style.space(8) : Style.space(6)
    anchors.rightMargin: Style.space(6)
    anchors.topMargin: Style.space(8)
    anchors.bottomMargin: Style.space(8)
    spacing: Style.space(6)

    Rectangle {
      width: parent.width
      height: Style.space(32)
      radius: Math.min(Style.cornerRadius, 8)
      color: root.myCardActive
        ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.16)
        : "transparent"
      Text {
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Style.space(8)
        anchors.rightMargin: Style.space(8)
        text: "My Card"
        textFormat: Text.PlainText
        color: root.textColor
        elide: Text.ElideRight
        font.family: Style.font.family
        font.pixelSize: Style.font.body
        font.bold: root.myCardActive
      }
      MouseArea {
        anchors.fill: parent
        onClicked: root.myCardRequested()
      }
    }

    Rectangle {
      width: parent.width
      height: 1
      color: Qt.rgba(root.textColor.r, root.textColor.g, root.textColor.b, 0.08)
    }

    TextField {
      id: nameField
      visible: root.naming
      width: parent.width
      placeholderText: "Group name"
      onAccepted: root.nameAccepted()
    }

    ListView {
      id: list
      width: parent.width
      height: Math.max(Style.space(80), parent.height - Style.space(40) - (root.naming ? Style.space(40) : 0))
      model: root.groups
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      currentIndex: root.myCardActive ? -1 : root.cursor
      spacing: Style.space(2)
      ScrollBar.vertical: ScrollBar {
        policy: list.contentHeight > list.height ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
      }
      delegate: Rectangle {
        required property var modelData
        required property int index
        width: list.width
        height: Style.space(32)
        radius: Math.min(Style.cornerRadius, 8)
        color: !root.myCardActive && index === root.cursor
          ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.16)
          : "transparent"
        Text {
          anchors.verticalCenter: parent.verticalCenter
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.leftMargin: Style.space(8)
          anchors.rightMargin: Style.space(8)
          text: String(modelData)
          textFormat: Text.PlainText
          color: root.textColor
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        MouseArea {
          anchors.fill: parent
          onClicked: root.activated(index)
        }
      }
    }
  }
}
