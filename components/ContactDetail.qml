import QtQuick
import qs.Commons
import qs.Ui

Item {
  id: root
  property var contact: null
  property color textColor: Color.foreground
  property color dimColor: Color.muted
  property color accent: Color.accent
  property color background: Color.background
  signal editRequested()
  signal deleteRequested()

  readonly property string name: contact ? String(contact.fn || "") : ""
  readonly property string orgLine: {
    if (!contact) return ""
    var t = String(contact.title || "")
    var o = String(contact.org || "")
    if (t && o) return t + " · " + o
    return t || o
  }

  function linesOf(arr, key) {
    if (!arr || !arr.length) return []
    var out = []
    for (var i = 0; i < arr.length; i++) {
      var v = arr[i] && arr[i][key] ? String(arr[i][key]) : ""
      var t = arr[i] && arr[i].type ? String(arr[i].type) : ""
      if (v) out.push({ type: t, value: v })
    }
    return out
  }

  Column {
    anchors.fill: parent
    anchors.margins: Style.space(20)
    spacing: Style.space(16)
    visible: !!root.contact

    Row {
      spacing: Style.space(16)
      Avatar {
        name: root.name
        hasPhoto: !!(root.contact && root.contact.has_photo)
        photoB64: root.contact && root.contact.photo_b64 ? String(root.contact.photo_b64) : ""
        size: Style.space(72)
        foreground: root.textColor
        background: root.background
        accent: root.accent
      }
      Column {
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.space(4)
        Text {
          text: root.name.length ? root.name : "Unnamed"
          textFormat: Text.PlainText
          color: root.textColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          font.bold: true
        }
        Text {
          visible: root.orgLine.length > 0
          text: root.orgLine
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        Text {
          visible: !!(root.contact && root.contact.nickname)
          text: root.contact ? String(root.contact.nickname) : ""
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
      }
    }

    Repeater {
      model: root.linesOf(root.contact ? root.contact.phones : [], "value")
      delegate: Row {
        spacing: Style.space(12)
        Text {
          width: Style.space(72)
          text: String(modelData.type || "phone")
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        Text {
          text: String(modelData.value || "")
          textFormat: Text.PlainText
          color: root.textColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
      }
    }

    Repeater {
      model: root.linesOf(root.contact ? root.contact.emails : [], "value")
      delegate: Row {
        spacing: Style.space(12)
        Text {
          width: Style.space(72)
          text: String(modelData.type || "email")
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        Text {
          text: String(modelData.value || "")
          textFormat: Text.PlainText
          color: root.textColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
      }
    }

    Text {
      visible: !!(root.contact && root.contact.note)
      width: parent.width
      text: root.contact ? String(root.contact.note) : ""
      textFormat: Text.PlainText
      wrapMode: Text.Wrap
      color: root.textColor
      font.family: Style.font.family
      font.pixelSize: Style.font.body
    }

    Row {
      spacing: Style.space(8)
      Button {
        text: "Edit"
        onClicked: root.editRequested()
      }
      Button {
        text: "Delete"
        onClicked: root.deleteRequested()
      }
    }
  }
}
