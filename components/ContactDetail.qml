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

  clip: true
  contentWidth: width
  contentHeight: col.implicitHeight
  boundsBehavior: Flickable.StopAtBounds
  flickableDirection: Flickable.VerticalFlick

  readonly property string name: contact ? String(contact.fn || "") : ""
  readonly property string orgLine: {
    if (!contact) return ""
    var t = String(contact.title || "")
    var r = String(contact.role || "")
    var o = String(contact.org || "")
    var d = String(contact.department || "")
    var job = t
    if (t && r) job = t + " · " + r
    else if (r) job = r
    var place = o
    if (o && d) place = o + " · " + d
    else if (d) place = d
    if (job && place) return job + " · " + place
    return job || place
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

  function fieldRows() {
    if (!root.contact) return []
    var c = root.contact
    var out = []
    function addAll(arr, fallback) {
      var rows = root.linesOf(arr, "value")
      for (var i = 0; i < rows.length; i++)
        out.push({ type: rows[i].type || fallback, value: rows[i].value })
    }
    addAll(c.phones, "phone")
    addAll(c.emails, "email")
    var addrs = root.addressLines()
    for (var a = 0; a < addrs.length; a++) out.push(addrs[a])
    addAll(c.urls, "web")
    addAll(c.ims, "im")
    addAll(c.socials, "social")
    addAll(c.related, "related")
    if (c.bday) out.push({ type: "birthday", value: String(c.bday) })
    if (c.anniversary) out.push({ type: "anniversary", value: String(c.anniversary) })
    addAll(c.dates, "date")
    if (c.groups && c.groups.length) {
      var g = []
      for (var i = 0; i < c.groups.length; i++) g.push(String(c.groups[i]))
      out.push({ type: "groups", value: g.join(" · ") })
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
      model: root.fieldRows()
      delegate: Row {
        spacing: Style.space(12)
        width: col.width - Style.space(40)
        Text {
          width: Style.space(88)
          text: String(modelData.type || "")
          textFormat: Text.PlainText
          color: root.dimColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          wrapMode: Text.Wrap
        }
        Text {
          width: parent.width - Style.space(100)
          text: String(modelData.value || "")
          textFormat: Text.PlainText
          wrapMode: Text.Wrap
          color: root.textColor
          font.family: Style.font.family
          font.pixelSize: Style.font.body
        }
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
  }
}
