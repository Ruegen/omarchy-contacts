import QtQuick
import qs.Commons
import qs.Ui

Flickable {
  id: root
  property var draft: ({})
  property color textColor: Color.foreground
  property color dimColor: Color.muted
  signal saveRequested()
  signal cancelRequested()

  clip: true
  contentWidth: width
  contentHeight: col.implicitHeight
  boundsBehavior: Flickable.StopAtBounds
  flickableDirection: Flickable.VerticalFlick
  Keys.forwardTo: [firstField]

  function takeFocus() { firstField.forceActiveFocus() }

  function val(obj, path, fallback) {
    if (!obj) return fallback
    return obj[path] !== undefined && obj[path] !== null ? obj[path] : fallback
  }

  function phoneVal(i) {
    var p = root.draft && root.draft.phones ? root.draft.phones[i] : null
    return p && p.value ? String(p.value) : ""
  }
  function phoneType(i, fallback) {
    var p = root.draft && root.draft.phones ? root.draft.phones[i] : null
    return p && p.type ? String(p.type) : fallback
  }
  function emailVal(i) {
    var p = root.draft && root.draft.emails ? root.draft.emails[i] : null
    return p && p.value ? String(p.value) : ""
  }

  function collect() {
    var d = Object.assign({}, root.draft)
    d.first = firstField.text
    d.last = lastField.text
    d.nickname = nickField.text
    d.org = orgField.text
    d.title = titleField.text
    d.note = noteField.text
    d.bday = bdayField.text
    d.phones = []
    if (phoneField.text.trim().length)
      d.phones.push({ type: phoneTypeField.text.trim() || "cell", value: phoneField.text })
    if (phone2Field.text.trim().length)
      d.phones.push({ type: phone2TypeField.text.trim() || "work", value: phone2Field.text })
    var extraPhones = root.draft && root.draft.phones ? root.draft.phones : []
    for (var i = 2; i < extraPhones.length; i++) d.phones.push(extraPhones[i])
    d.emails = []
    if (emailField.text.trim().length)
      d.emails.push({ type: emailTypeField.text.trim() || "work", value: emailField.text })
    if (email2Field.text.trim().length)
      d.emails.push({ type: "home", value: email2Field.text })
    var extraEmails = root.draft && root.draft.emails ? root.draft.emails : []
    for (var j = 2; j < extraEmails.length; j++) d.emails.push(extraEmails[j])
    return d
  }

  Column {
    id: col
    width: root.width
    spacing: Style.space(10)
    leftPadding: Style.space(20)
    rightPadding: Style.space(20)
    topPadding: Style.space(16)
    bottomPadding: Style.space(20)

    Text {
      text: root.draft && root.draft.uid ? "Edit contact" : "New contact"
      textFormat: Text.PlainText
      color: root.textColor
      font.family: Style.font.family
      font.pixelSize: Style.font.body
      font.bold: true
    }

    Text { text: "First name"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    TextField { id: firstField; width: col.width - Style.space(40); text: String(root.val(root.draft, "first", "")) }
    Text { text: "Last name"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    TextField { id: lastField; width: firstField.width; text: String(root.val(root.draft, "last", "")) }
    Text { text: "Nickname"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    TextField { id: nickField; width: firstField.width; text: String(root.val(root.draft, "nickname", "")) }
    Text { text: "Organization"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    TextField { id: orgField; width: firstField.width; text: String(root.val(root.draft, "org", "")) }
    Text { text: "Job title"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    TextField { id: titleField; width: firstField.width; text: String(root.val(root.draft, "title", "")) }
    Text { text: "Phone (type, then number)"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    Row {
      spacing: Style.space(8)
      TextField { id: phoneTypeField; width: Style.space(90); text: root.phoneType(0, "cell") }
      TextField { id: phoneField; width: firstField.width - Style.space(98); text: root.phoneVal(0) }
    }
    Row {
      spacing: Style.space(8)
      TextField { id: phone2TypeField; width: Style.space(90); text: root.phoneType(1, "work") }
      TextField { id: phone2Field; width: firstField.width - Style.space(98); text: root.phoneVal(1) }
    }
    Text { text: "Email (type, then address)"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    Row {
      spacing: Style.space(8)
      TextField { id: emailTypeField; width: Style.space(90); text: "work" }
      TextField { id: emailField; width: firstField.width - Style.space(98); text: root.emailVal(0) }
    }
    TextField { id: email2Field; width: firstField.width; text: root.emailVal(1) }
    Text { text: "Birthday"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    TextField { id: bdayField; width: firstField.width; text: String(root.val(root.draft, "bday", "")) }
    Text { text: "Notes"; textFormat: Text.PlainText; color: root.dimColor; font.family: Style.font.family; font.pixelSize: Style.font.body }
    TextField { id: noteField; width: firstField.width; text: String(root.val(root.draft, "note", "")) }

    Row {
      spacing: Style.space(8)
      Button { text: "Save"; foreground: root.dimColor; fontSize: Style.font.caption; onClicked: root.saveRequested() }
      Button { text: "Cancel"; foreground: root.dimColor; fontSize: Style.font.caption; onClicked: root.cancelRequested() }
    }
  }
}
