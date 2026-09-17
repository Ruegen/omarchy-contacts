import QtQuick
import qs.Commons
import qs.Ui

Flickable {
  id: root
  property var contact: null
  property color textColor: Color.foreground
  property color dimColor: Color.muted
  property color accent: Color.accent
  property color background: Color.background
  signal editRequested()
  signal deleteRequested()
  signal exportRequested()
  signal emailRequested()

  clip: true
  contentWidth: width
  contentHeight: col.implicitHeight
  boundsBehavior: Flickable.StopAtBounds
  flickableDirection: Flickable.VerticalFlick

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

  function addressLine(a) {
    if (!a) return ""
    var parts = [a.street, a.city, a.region, a.postal, a.country]
    var out = []
    for (var i = 0; i < parts.length; i++) {
      var s = String(parts[i] || "").trim()
      if (s.length) out.push(s)
    }
    return out.join(", ")
  }

  function addressLines() {
    var arr = root.contact && root.contact.addresses ? root.contact.addresses : []
    var out = []
    for (var i = 0; i < arr.length; i++) {
      var line = root.addressLine(arr[i])
      if (line) out.push({ type: arr[i] && arr[i].type ? String(arr[i].type) : "address", value: line })
    }
    return out
  }

  Column {
    id: col
    width: root.width
    visible: !!root.contact
    topPadding: Style.space(20)
    leftPadding: Style.space(20)
    rightPadding: Style.space(20)
    bottomPadding: Style.space(20)
    spacing: Style.space(16)

    Row {
      spacing: Style.space(16)
      Avatar {
        name: root.name
        hasPhoto: !!(root.contact && root.contact.has_photo)
        photoB64: root.contact && root.contact.photo_b64 ? String(root.contact.photo_b64) : ""
        photoFile: root.contact && root.contact.photo_file ? String(root.contact.photo_file) : ""
        size: Style.space(72)
        foreground: root.textColor
        background: root.background
        accent: root.accent
      }
      Column {
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(40, col.width - Style.space(40) - Style.space(88))
        spacing: Style.space(4)
        Text {
          width: parent.width
          text: root.name.length ? root.name : "Unnamed"
          textFormat: Text.PlainText
          color: root.textColor
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          font.bold: true
        }
        Text {
          visible: root.orgLine.length > 0
          width: parent.width
          text: root.orgLine
          textFormat: Text.PlainText
          color: root.dimColor
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        Text {
          visible: !!(root.contact && root.contact.nickname)
          width: parent.width
          text: root.contact ? String(root.contact.nickname) : ""
          textFormat: Text.PlainText
          color: root.dimColor
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
      }
    }

    Repeater {
      model: root.linesOf(root.contact ? root.contact.phones : [], "value")
      delegate: Row {
        spacing: Style.space(12)
        width: col.width - Style.space(40)
        Text {
          width: Style.space(72)
          text: String(modelData.type || "phone")
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        Text {
          width: parent.width - Style.space(84)
          text: String(modelData.value || "")
          textFormat: Text.PlainText
          wrapMode: Text.Wrap
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
        width: col.width - Style.space(40)
        Text {
          width: Style.space(72)
          text: String(modelData.type || "email")
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        Text {
          width: parent.width - Style.space(84)
          text: String(modelData.value || "")
          textFormat: Text.PlainText
          wrapMode: Text.Wrap
          color: root.textColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
      }
    }

    Repeater {
      model: root.addressLines()
      delegate: Row {
        spacing: Style.space(12)
        width: col.width - Style.space(40)
        Text {
          width: Style.space(72)
          text: String(modelData.type || "address")
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        Text {
          width: parent.width - Style.space(84)
          text: String(modelData.value || "")
          textFormat: Text.PlainText
          wrapMode: Text.Wrap
          color: root.textColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
      }
    }

    Repeater {
      model: root.linesOf(root.contact ? root.contact.urls : [], "value")
      delegate: Row {
        spacing: Style.space(12)
        width: col.width - Style.space(40)
        Text {
          width: Style.space(72)
          text: String(modelData.type || "web")
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
        Text {
          width: parent.width - Style.space(84)
          text: String(modelData.value || "")
          textFormat: Text.PlainText
          wrapMode: Text.Wrap
          color: root.textColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
      }
    }

    Row {
      visible: !!(root.contact && root.contact.bday)
      spacing: Style.space(12)
      width: col.width - Style.space(40)
      Text {
        width: Style.space(72)
        text: "birthday"
        textFormat: Text.PlainText
        color: root.dimColor
        font.family: Style.font.family
        font.pixelSize: Style.font.body
      }
      Text {
        width: parent.width - Style.space(84)
        text: root.contact ? String(root.contact.bday) : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: root.textColor
        font.family: Style.font.family
        font.pixelSize: Style.font.body
      }
    }

    Text {
      visible: !!(root.contact && root.contact.note)
      width: col.width - Style.space(40)
      text: root.contact ? String(root.contact.note) : ""
      textFormat: Text.PlainText
      wrapMode: Text.Wrap
      color: root.textColor
      font.family: Style.font.family
      font.pixelSize: Style.font.body
    }

    Text {
      visible: !!(root.contact && root.contact.groups && root.contact.groups.length)
      width: col.width - Style.space(40)
      text: {
        if (!root.contact || !root.contact.groups) return ""
        var g = []
        for (var i = 0; i < root.contact.groups.length; i++) g.push(String(root.contact.groups[i]))
        return g.join(" · ")
      }
      textFormat: Text.PlainText
      wrapMode: Text.Wrap
      color: root.dimColor
      font.family: Style.font.family
      font.pixelSize: Style.font.body
    }

    Flow {
      width: col.width - Style.space(40)
      spacing: Style.space(8)
      Button {
        text: "Edit"
        bordered: true
        onClicked: root.editRequested()
      }
      Button {
        text: "Delete"
        bordered: true
        onClicked: root.deleteRequested()
      }
      Button {
        text: "Export"
        bordered: true
        onClicked: root.exportRequested()
      }
      Button {
        text: "Mail"
        bordered: true
        onClicked: root.emailRequested()
      }
    }
  }
}
