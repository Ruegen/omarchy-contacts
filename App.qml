import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "components"
import "keys/Keymap.js" as Keymap

// Contacts window. The shell loads this when the plugin is summoned and
// calls open()/close(); the FloatingWindow follows.
Item {
  id: root

  property var shell: null
  property var manifest: null
  property var service: null
  property bool opened: false
  property bool closingFromHost: false

  readonly property var svc: {
    if (service) return service
    if (shell && typeof shell.serviceFor === "function")
      return shell.serviceFor("omarchy-contacts")
    return null
  }

  readonly property color foreground: Color.foreground
  readonly property color background: Color.background
  readonly property color accent: Color.accent
  readonly property color muted: Color.muted
  readonly property color popupBackground: Color.popups.background
  readonly property color dim: Qt.rgba(
    foreground.r * 0.68 + background.r * 0.32,
    foreground.g * 0.68 + background.g * 0.32,
    foreground.b * 0.68 + background.b * 0.32, 1)

  property bool searchFocused: false
  property bool helpOpen: false
  property bool editing: false
  property bool setupOpen: false
  property bool confirmDelete: false
  property var draft: ({})
  property int cursor: 0
  property string searchText: ""
  property string appleIdDraft: ""
  property string passwordDraft: ""

  readonly property var contacts: svc && svc.contacts ? svc.contacts : []
  readonly property var current: svc ? svc.current : null
  readonly property var keys: Keymap.merge(svc && svc.keys ? svc.keys : {})
  readonly property string keyContext: Keymap.contextFor({
    helpOpen: helpOpen,
    editing: editing,
    searchFocused: searchFocused,
    setupOpen: setupOpen
  })
  readonly property var sync: svc && svc.sync ? svc.sync : ({})
  readonly property bool syncing: !!(sync && (sync.syncing || (sync.progress && sync.progress.phase && sync.progress.phase !== "idle" && sync.progress.phase !== "")))
  readonly property string notice: svc ? String(svc.notice || svc.errorText || "") : ""

  function open() {
    closingFromHost = false
    opened = true
    if (svc && typeof svc.list === "function") svc.list(searchText)
    if (svc && typeof svc.ensureRunning === "function") svc.ensureRunning()
    Qt.callLater(function() { if (focusScope) focusScope.forceActiveFocus() })
  }
  function close() {
    closingFromHost = true
    opened = false
    helpOpen = false
    searchFocused = false
    confirmDelete = false
  }
  function requestClose() {
    if (shell && typeof shell.hide === "function") shell.hide("omarchy-contacts")
    else close()
  }

  function selectedUid() {
    if (!contacts.length || cursor < 0 || cursor >= contacts.length) return ""
    return String(contacts[cursor].uid || "")
  }

  function selectCurrent() {
    var uid = selectedUid()
    if (uid && svc && typeof svc.get === "function") svc.get(uid)
  }

  function moveCursor(dy) {
    if (!contacts.length) return
    cursor = Math.max(0, Math.min(contacts.length - 1, cursor + dy))
    selectCurrent()
  }

  function startNew() {
    editing = true
    setupOpen = false
    helpOpen = false
    draft = { first: "", last: "", nickname: "", org: "", title: "", note: "", phones: [], emails: [] }
    Qt.callLater(function() { if (editor) editor.takeFocus() })
  }

  function startEdit() {
    if (!current) selectCurrent()
    if (!svc || !svc.current) return
    editing = true
    draft = Object.assign({}, svc.current)
    Qt.callLater(function() { if (editor) editor.takeFocus() })
  }

  function saveDraft() {
    if (!svc || !editor) return
    var c = editor.collect()
    svc.save(c, function(res) {
      if (res && res.ok) {
        editing = false
        draft = ({})
        focusScope.forceActiveFocus()
      }
    })
  }

  function deleteSelected() {
    var uid = selectedUid()
    if (!uid || !svc) return
    svc.remove(uid, function() {
      confirmDelete = false
      if (cursor >= contacts.length) cursor = Math.max(0, contacts.length - 1)
    })
  }

  function applySearch() {
    if (svc && typeof svc.list === "function") svc.list(searchText)
    cursor = 0
  }

  function pickImport(csv) {
    picker.csv = !!csv
    picker.save = false
    picker.command = ["/usr/bin/zenity", "--file-selection",
      csv ? "--file-filter=CSV | *.csv" : "--file-filter=vCard | *.vcf *.vcard"]
    picker.running = true
  }

  function pickExport(csv) {
    picker.csv = !!csv
    picker.save = true
    picker.command = ["/usr/bin/zenity", "--file-selection", "--save", "--confirm-overwrite",
      "--filename=" + (csv ? "contacts.csv" : "contacts.vcf")]
    picker.running = true
  }

  function handleDrop(urls) {
    if (!svc) return
    for (var i = 0; i < urls.length; i++) {
      var p = String(urls[i] || "").replace(/^file:\/\//, "")
      try { p = decodeURIComponent(p) } catch (e) {}
      var lower = p.toLowerCase()
      if (lower.endsWith(".vcf") || lower.endsWith(".vcard"))
        svc.importFile(p, false, function() {})
      else if (lower.endsWith(".csv"))
        svc.importFile(p, true, function() {})
    }
  }

  function dispatchKey(event) {
    if (Keymap.matchChord(event, keys.help) && keyContext !== "edit") {
      helpOpen = !helpOpen
      event.accepted = true
      return
    }
    if (Keymap.matchChord(event, keys.escape)) {
      if (helpOpen) { helpOpen = false; event.accepted = true; return }
      if (confirmDelete) { confirmDelete = false; event.accepted = true; return }
      if (editing) { editing = false; event.accepted = true; return }
      if (setupOpen) { setupOpen = false; event.accepted = true; return }
      if (searchFocused) {
        searchFocused = false
        searchText = ""
        applySearch()
        event.accepted = true
        return
      }
      requestClose()
      event.accepted = true
      return
    }
    if (keyContext === "help") { event.accepted = true; return }
    if (keyContext === "edit") {
      if (Keymap.matchChord(event, keys.save)) { saveDraft(); event.accepted = true }
      return
    }
    if (keyContext === "setup") return
    if (!searchFocused && Keymap.matchChord(event, keys.search)) {
      searchFocused = true
      searchField.forceActiveFocus()
      event.accepted = true
      return
    }
    if (searchFocused && (event.key === Qt.Key_Down || event.key === Qt.Key_Up)) {
      searchFocused = false
      moveCursor(event.key === Qt.Key_Down ? 1 : -1)
      event.accepted = true
      return
    }
    if (searchFocused) return
    if (Keymap.matchChord(event, keys.down) || Keymap.matchChord(event, keys.downAlt)) {
      moveCursor(1); event.accepted = true; return
    }
    if (Keymap.matchChord(event, keys.up) || Keymap.matchChord(event, keys.upAlt)) {
      moveCursor(-1); event.accepted = true; return
    }
    if (Keymap.matchChord(event, keys.open)) {
      selectCurrent(); event.accepted = true; return
    }
    if (Keymap.matchChord(event, keys.newContact)) { startNew(); event.accepted = true; return }
    if (Keymap.matchChord(event, keys.edit)) { startEdit(); event.accepted = true; return }
    if (Keymap.matchChord(event, keys.deleteContact)) { confirmDelete = true; event.accepted = true; return }
    if (Keymap.matchChord(event, keys.sync)) {
      if (svc) svc.syncNow(function() {})
      event.accepted = true
      return
    }
    if (Keymap.matchChord(event, keys.importFile)) { pickImport(false); event.accepted = true; return }
    if (Keymap.matchChord(event, keys.exportFile)) { pickExport(false); event.accepted = true; return }
  }

  onContactsChanged: {
    if (cursor >= contacts.length) cursor = Math.max(0, contacts.length - 1)
  }

  Process {
    id: picker
    property bool csv: false
    property bool save: false
    stdout: SplitParser {
      splitMarker: "\n"
      onRead: function(data) {
        var path = String(data || "").replace(/\r/g, "").trim()
        if (!path || path.length > 4096) return
        if (picker.save) {
          if (root.svc) root.svc.exportFile(path, [], picker.csv, function() {})
        } else {
          if (root.svc) root.svc.importFile(path, picker.csv, function() {})
        }
      }
    }
  }

  FloatingWindow {
    id: window
    visible: root.opened
    title: "Contacts"
    color: root.background
    implicitWidth: Style.space(920)
    implicitHeight: Style.space(620)
    minimumSize: Qt.size(Style.space(720), Style.space(480))

    onVisibleChanged: {
      if (!visible && root.opened && !root.closingFromHost) root.requestClose()
    }

    FocusScope {
      id: focusScope
      anchors.fill: parent
      focus: true
      Keys.priority: Keys.AfterItem
      Keys.onPressed: function(event) { root.dispatchKey(event) }

      DropArea {
        anchors.fill: parent
        onDropped: function(drop) {
          if (drop.hasUrls) root.handleDrop(drop.urls)
        }
      }

      Column {
        anchors.fill: parent
        spacing: 0

        Rectangle {
          width: parent.width
          height: Style.space(52)
          color: root.popupBackground

          Row {
            anchors.fill: parent
            anchors.leftMargin: Style.space(16)
            anchors.rightMargin: Style.space(16)
            spacing: Style.space(12)

            Text {
              anchors.verticalCenter: parent.verticalCenter
              text: "Contacts"
              textFormat: Text.PlainText
              color: root.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.body
              font.bold: true
            }

            TextField {
              id: searchField
              anchors.verticalCenter: parent.verticalCenter
              width: Style.space(240)
              placeholderText: "Search"
              text: root.searchText
              onActiveFocusChanged: root.searchFocused = activeFocus
              onTextChanged: {
                root.searchText = text
                root.applySearch()
              }
            }

            Item { width: 1; height: 1 }

            Text {
              anchors.verticalCenter: parent.verticalCenter
              visible: root.syncing
              text: {
                var p = root.sync && root.sync.progress ? root.sync.progress : ({})
                var done = Number(p.done) || 0
                var total = Number(p.total) || 0
                var phase = String(p.phase || "sync")
                return total > 0 ? ("Syncing " + phase + " " + done + "/" + total) : "Syncing…"
              }
              textFormat: Text.PlainText
              color: root.accent
              font.family: Style.font.family
              font.pixelSize: Style.font.body
            }

            Text {
              anchors.verticalCenter: parent.verticalCenter
              visible: root.notice.length > 0 && !root.syncing
              text: root.svc ? root.svc.plain(root.notice) : ""
              textFormat: Text.PlainText
              color: root.dim
              font.family: Style.font.family
              font.pixelSize: Style.font.body
              elide: Text.ElideRight
            }

            Item { width: Style.space(16); height: 1 }

            Button {
              anchors.verticalCenter: parent.verticalCenter
              text: "iCloud"
              onClicked: root.setupOpen = !root.setupOpen
            }
          }
        }

        Rectangle {
          width: parent.width
          height: 1
          color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
        }

        Row {
          width: parent.width
          height: parent.height - Style.space(52) - 1

          Rectangle {
            width: Style.space(320)
            height: parent.height
            color: root.background

            ContactList {
              id: list
              anchors.fill: parent
              anchors.margins: Style.space(8)
              contacts: root.contacts
              cursor: root.cursor
              textColor: root.foreground
              dimColor: root.dim
              accent: root.accent
              background: root.background
              visible: root.contacts.length > 0 && !(root.svc && root.svc.daemonMissing)
              onActivated: function(uid) {
                for (var i = 0; i < root.contacts.length; i++) {
                  if (String(root.contacts[i].uid) === uid) root.cursor = i
                }
                root.selectCurrent()
              }
            }

            EmptyState {
              anchors.fill: parent
              visible: root.contacts.length === 0 && !(root.svc && root.svc.daemonMissing)
              textColor: root.foreground
              dimColor: root.dim
              title: "No contacts yet"
              body: "Press n to add someone, or drop a vCard on this window."
            }

            EmptyState {
              anchors.fill: parent
              visible: !!(root.svc && root.svc.daemonMissing)
              textColor: root.foreground
              dimColor: root.dim
              title: "Helper is not built"
              body: "In the plugin folder run make daemon, then open Contacts again."
            }
          }

          Rectangle {
            width: 1
            height: parent.height
            color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
          }

          Rectangle {
            width: parent.width - Style.space(321)
            height: parent.height
            color: root.background

            ContactDetail {
              anchors.fill: parent
              visible: !root.editing && !root.setupOpen && !!root.current
              contact: root.current
              textColor: root.foreground
              dimColor: root.dim
              accent: root.accent
              background: root.background
              onEditRequested: root.startEdit()
              onDeleteRequested: root.confirmDelete = true
            }

            EmptyState {
              anchors.fill: parent
              visible: !root.editing && !root.setupOpen && !root.current
              textColor: root.foreground
              dimColor: root.dim
              title: "Select a contact"
              body: "Use j and k to move, Enter to open, e to edit."
            }

            ContactEditor {
              id: editor
              anchors.fill: parent
              visible: root.editing
              draft: root.draft
              textColor: root.foreground
              dimColor: root.dim
              onSaveRequested: root.saveDraft()
              onCancelRequested: root.editing = false
            }

            Column {
              visible: root.setupOpen && !root.editing
              anchors.fill: parent
              anchors.margins: Style.space(24)
              spacing: Style.space(10)

              Text {
                text: "iCloud"
                textFormat: Text.PlainText
                color: root.foreground
                font.family: Style.font.family
                font.pixelSize: Style.font.body
                font.bold: true
              }
              Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "Use an app-specific password from appleid.apple.com → Sign-In & Security → App-Specific Passwords. Your regular Apple ID password is rejected."
                textFormat: Text.PlainText
                color: root.dim
                font.family: Style.font.family
                font.pixelSize: Style.font.body
              }
              Text { text: "Apple ID"; textFormat: Text.PlainText; color: root.dim; font.family: Style.font.family; font.pixelSize: Style.font.body }
              TextField {
                id: appleIdField
                width: Math.min(parent.width, Style.space(360))
                text: root.sync && root.sync.apple_id ? String(root.sync.apple_id) : ""
              }
              Text { text: "App-specific password"; textFormat: Text.PlainText; color: root.dim; font.family: Style.font.family; font.pixelSize: Style.font.body }
              TextField {
                id: passwordField
                width: appleIdField.width
                password: true
              }
              Row {
                spacing: Style.space(8)
                Button {
                  text: "Save and sync"
                  onClicked: {
                    if (!root.svc) return
                    root.svc.configureIcloud(appleIdField.text, passwordField.text, function(res) {
                      passwordField.text = ""
                      if (res && res.ok) {
                        root.setupOpen = false
                        root.svc.syncNow(function() {})
                      }
                    })
                  }
                }
                Button {
                  text: "Cancel"
                  onClicked: {
                    passwordField.text = ""
                    root.setupOpen = false
                  }
                }
              }
            }

            Rectangle {
              visible: root.confirmDelete
              anchors.fill: parent
              color: Qt.rgba(root.background.r, root.background.g, root.background.b, 0.92)
              Column {
                anchors.centerIn: parent
                spacing: Style.space(12)
                Text {
                  text: "Delete this contact?"
                  textFormat: Text.PlainText
                  color: root.foreground
                  font.family: Style.font.family
                  font.pixelSize: Style.font.body
                }
                Row {
                  spacing: Style.space(8)
                  Button { text: "Delete"; onClicked: root.deleteSelected() }
                  Button { text: "Cancel"; onClicked: root.confirmDelete = false }
                }
              }
            }
          }
        }
      }

      ShortcutOverlay {
        anchors.fill: parent
        visible: root.helpOpen
        textColor: root.foreground
        backgroundColor: root.background
        dimColor: root.dim
        bindings: root.keys
        onDismissed: root.helpOpen = false
      }
    }
  }
}
