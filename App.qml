import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "components"
import "keys/Keymap.js" as Keymap

// Contacts window. A standalone Quickshell app: the launcher starts this
// window, and closing it leaves the desktop.
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
  property bool settingsOpen: false
  property string selectedOption: ""
  property string selectedAccount: ""
  property int settingsColumn: 0
  property int optionsCursor: 0
  property int accountsCursor: 0
  property bool confirmDelete: false
  property bool setupBusy: false
  property string setupError: ""
  property var draft: ({})
  property int cursor: 0
  property string searchText: ""
  readonly property string passwordMask: "••••••••••••••••"

  readonly property var contacts: svc && svc.contacts ? svc.contacts : []
  readonly property var current: svc ? svc.current : null
  readonly property var keys: Keymap.merge(svc && svc.keys ? svc.keys : {})
  readonly property string keyContext: Keymap.contextFor({
    helpOpen: helpOpen,
    editing: editing,
    searchFocused: searchFocused,
    setupOpen: settingsOpen
  })
  readonly property var sync: svc && svc.sync ? svc.sync : ({})
  readonly property bool syncing: !!(sync && (sync.syncing || (sync.progress && sync.progress.phase && sync.progress.phase !== "idle" && sync.progress.phase !== "")))
  readonly property string notice: {
    if (svc && svc.errorText) return String(svc.errorText)
    if (sync && sync.progress && sync.progress.error) return String(sync.progress.error)
    if (svc) return String(svc.notice || "")
    return ""
  }
  readonly property bool accountsColumnOpen: settingsOpen && selectedOption === "accounts"
  readonly property bool icloudPaneOpen: settingsOpen && selectedAccount === "icloud"
  readonly property var optionsItems: [
    { id: "accounts", label: "Accounts", hint: "iCloud and other accounts" }
  ]
  readonly property var accountItems: [
    { id: "icloud", label: "iCloud", hint: (sync && sync.apple_id) ? String(sync.apple_id) : "Not signed in" }
  ]

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
    settingsOpen = false
    selectedOption = ""
    selectedAccount = ""
  }
  function requestClose() {
    closingFromHost = true
    opened = false
    Qt.quit()
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
    closeSettings()
    helpOpen = false
    draft = { first: "", last: "", nickname: "", org: "", title: "", note: "", phones: [], emails: [] }
    Qt.callLater(function() { if (editor) editor.takeFocus() })
  }

  function openSettings() {
    editing = false
    helpOpen = false
    settingsOpen = true
    selectedOption = ""
    selectedAccount = ""
    settingsColumn = 0
    optionsCursor = 0
    accountsCursor = 0
    setupError = ""
    setupBusy = false
  }

  function closeSettings() {
    settingsOpen = false
    selectedOption = ""
    selectedAccount = ""
    settingsColumn = 0
    optionsCursor = 0
    accountsCursor = 0
    setupError = ""
    setupBusy = false
  }

  function settingsBack() {
    if (selectedAccount.length > 0) {
      selectedAccount = ""
      settingsColumn = 1
      setupError = ""
      return
    }
    if (selectedOption.length > 0) {
      selectedOption = ""
      settingsColumn = 0
      return
    }
    closeSettings()
  }

  function chooseOption(id, index) {
    optionsCursor = index
    selectedOption = id
    selectedAccount = ""
    settingsColumn = 1
    accountsCursor = 0
    setupError = ""
  }

  function settingsEnter() {
    if (settingsColumn <= 0) {
      var item = optionsItems[optionsCursor]
      if (item) chooseOption(item.id, optionsCursor)
      return
    }
    if (settingsColumn === 1) {
      var acc = accountItems[accountsCursor]
      if (acc && acc.id === "icloud") openIcloudPage()
    }
  }

  function fillPasswordDots() {
    if (!passwordField) return
    passwordField.text = root.passwordMask
  }

  function passwordForSave() {
    var typed = passwordField ? String(passwordField.text || "") : ""
    if (typed === root.passwordMask) return ""
    if (typed.length === 0 && root.sync && root.sync.has_password) return ""
    return typed
  }

  function openIcloudPage() {
    selectedAccount = "icloud"
    settingsColumn = 2
    setupError = ""
    Qt.callLater(function() {
      if (root.sync && root.sync.apple_id)
        appleIdField.text = String(root.sync.apple_id)
      if (root.sync && root.sync.has_password)
        root.fillPasswordDots()
      if (!appleIdField.text) appleIdField.forceActiveFocus()
      else if (!passwordField.text || passwordField.text === root.passwordMask) {
        if (!root.sync || !root.sync.has_password) passwordField.forceActiveFocus()
      } else {
        passwordField.forceActiveFocus()
      }
    })
  }

  function moveSettingsCursor(dy) {
    if (settingsColumn <= 0) {
      if (!optionsItems.length) return
      optionsCursor = Math.max(0, Math.min(optionsItems.length - 1, optionsCursor + dy))
      return
    }
    if (settingsColumn === 1) {
      if (!accountItems.length) return
      accountsCursor = Math.max(0, Math.min(accountItems.length - 1, accountsCursor + dy))
    }
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

  function saveIcloud() {
    if (!root.svc || root.setupBusy) return
    root.setupError = ""
    root.setupBusy = true
    var appleId = appleIdField.text
    var password = root.passwordForSave()
    root.svc.configureIcloud(appleId, password, function(res) {
      if (!res || !res.ok) {
        root.setupBusy = false
        root.setupError = root.svc.plain(res && res.error ? res.error : "Could not save those details")
        return
      }
      root.fillPasswordDots()
      root.setupError = "Looking for your contacts on iCloud…"
      root.svc.syncNow(function(syncRes) {
        root.setupBusy = false
        if (syncRes && syncRes.ok) {
          root.setupError = ""
        } else {
          root.setupError = root.svc.plain(syncRes && syncRes.error ? syncRes.error : "Sync did not finish")
        }
      })
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
      if (settingsOpen) { settingsBack(); event.accepted = true; return }
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
    if (settingsOpen && selectedAccount !== "icloud") {
      if (Keymap.matchChord(event, keys.down) || Keymap.matchChord(event, keys.downAlt)) {
        moveSettingsCursor(1); event.accepted = true; return
      }
      if (Keymap.matchChord(event, keys.up) || Keymap.matchChord(event, keys.upAlt)) {
        moveSettingsCursor(-1); event.accepted = true; return
      }
      if (event.key === Qt.Key_Right || Keymap.matchChord(event, keys.open)) {
        settingsEnter(); event.accepted = true; return
      }
      if (event.key === Qt.Key_Left) {
        settingsBack(); event.accepted = true; return
      }
      event.accepted = true
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

            Button {
              anchors.verticalCenter: parent.verticalCenter
              iconText: "☰"
              tooltipText: "Options"
              selected: root.settingsOpen
              onClicked: {
                if (root.settingsOpen) root.closeSettings()
                else root.openSettings()
              }
            }

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

            Row {
              anchors.verticalCenter: parent.verticalCenter
              visible: root.syncing
              spacing: Style.space(8)

              Item {
                width: Style.space(14)
                height: Style.space(14)
                anchors.verticalCenter: parent.verticalCenter

                Canvas {
                  id: syncSpinner
                  anchors.fill: parent
                  antialiasing: true
                  readonly property color stroke: root.accent
                  onStrokeChanged: requestPaint()
                  onPaint: {
                    var ctx = getContext("2d")
                    ctx.reset()
                    var cx = width / 2
                    var cy = height / 2
                    var r = Math.max(1, Math.min(width, height) / 2 - 1.5)
                    ctx.strokeStyle = root.accent
                    ctx.lineWidth = 1.6
                    ctx.lineCap = "round"
                    ctx.beginPath()
                    ctx.arc(cx, cy, r, -Math.PI * 0.5, Math.PI * 0.9)
                    ctx.stroke()
                  }
                  onWidthChanged: requestPaint()
                  onHeightChanged: requestPaint()
                  Component.onCompleted: requestPaint()

                  RotationAnimator on rotation {
                    running: root.syncing
                    from: 0
                    to: 360
                    duration: 2400
                    loops: Animation.Infinite
                  }
                }
              }

              Text {
                anchors.verticalCenter: parent.verticalCenter
                text: {
                  var p = root.sync && root.sync.progress ? root.sync.progress : ({})
                  var done = Number(p.done) || 0
                  var total = Number(p.total) || 0
                  var phase = String(p.phase || "")
                  if (total > 0) return (phase === "photos" ? "Photos " : "Syncing ") + done + "/" + total
                  if (phase === "discover" || phase === "listing") return "Looking up iCloud…"
                  return "Syncing…"
                }
                textFormat: Text.PlainText
                color: root.accent
                font.family: Style.font.family
                font.pixelSize: Style.font.body
              }
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
            width: Style.space(280)
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
              visible: !root.settingsOpen && root.contacts.length > 0 && !(root.svc && root.svc.daemonMissing)
              onActivated: function(uid) {
                for (var i = 0; i < root.contacts.length; i++) {
                  if (String(root.contacts[i].uid) === uid) root.cursor = i
                }
                root.selectCurrent()
              }
            }

            EmptyState {
              anchors.fill: parent
              visible: !root.settingsOpen && root.contacts.length === 0 && !(root.svc && root.svc.daemonMissing)
              textColor: root.foreground
              dimColor: root.dim
              title: "No contacts yet"
              body: "Press n to add someone, or drop a vCard on this window."
            }

            EmptyState {
              anchors.fill: parent
              visible: !root.settingsOpen && !!(root.svc && root.svc.daemonMissing)
              textColor: root.foreground
              dimColor: root.dim
              title: "Helper is not built"
              body: "In the plugin folder run make daemon, then open Contacts again."
            }

            Column {
              visible: root.settingsOpen
              anchors.fill: parent
              anchors.margins: Style.space(12)
              spacing: Style.space(6)

              Text {
                text: "Options"
                textFormat: Text.PlainText
                color: root.foreground
                font.family: Style.font.family
                font.pixelSize: Style.font.body
                font.bold: true
              }

              Repeater {
                model: root.optionsItems
                delegate: Rectangle {
                  required property int index
                  required property var modelData
                  width: parent.width
                  height: Style.space(52)
                  radius: 6
                  color: {
                    if (modelData.id === root.selectedOption)
                      return Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.16)
                    if (root.settingsColumn === 0 && index === root.optionsCursor)
                      return Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.06)
                    return "transparent"
                  }
                  MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                      root.settingsColumn = 0
                      root.chooseOption(modelData.id, index)
                    }
                  }
                  Text {
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(12)
                    anchors.right: chevron1.left
                    anchors.rightMargin: Style.space(8)
                    text: modelData.label
                    textFormat: Text.PlainText
                    color: root.foreground
                    font.family: Style.font.family
                    font.pixelSize: Style.font.body
                    elide: Text.ElideRight
                  }
                  Text {
                    id: chevron1
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.right: parent.right
                    anchors.rightMargin: Style.space(12)
                    text: "›"
                    textFormat: Text.PlainText
                    color: root.dim
                    font.family: Style.font.family
                    font.pixelSize: Style.font.body
                  }
                }
              }
            }
          }

          Rectangle {
            width: 1
            height: parent.height
            color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
          }

          Rectangle {
            width: root.accountsColumnOpen ? Style.space(260) : 0
            height: parent.height
            visible: root.accountsColumnOpen
            color: root.background
            clip: true

            Column {
              anchors.fill: parent
              anchors.margins: Style.space(12)
              spacing: Style.space(6)

              Text {
                text: "Accounts"
                textFormat: Text.PlainText
                color: root.foreground
                font.family: Style.font.family
                font.pixelSize: Style.font.body
                font.bold: true
              }

              Repeater {
                model: root.accountItems
                delegate: Rectangle {
                  required property int index
                  required property var modelData
                  width: parent.width
                  height: Style.space(52)
                  radius: 6
                  color: {
                    if (modelData.id === root.selectedAccount)
                      return Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.16)
                    if (root.settingsColumn === 1 && index === root.accountsCursor)
                      return Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.06)
                    return "transparent"
                  }
                  MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                      root.accountsCursor = index
                      root.settingsColumn = 1
                      if (modelData.id === "icloud") root.openIcloudPage()
                    }
                  }
                  Column {
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(12)
                    anchors.right: chevron2.left
                    anchors.rightMargin: Style.space(8)
                    spacing: Style.space(2)
                    Text {
                      width: parent.width
                      text: modelData.label
                      textFormat: Text.PlainText
                      color: root.foreground
                      font.family: Style.font.family
                      font.pixelSize: Style.font.body
                      elide: Text.ElideRight
                    }
                    Text {
                      width: parent.width
                      text: modelData.hint
                      textFormat: Text.PlainText
                      color: root.dim
                      font.family: Style.font.family
                      font.pixelSize: Style.font.body
                      elide: Text.ElideRight
                    }
                  }
                  Text {
                    id: chevron2
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.right: parent.right
                    anchors.rightMargin: Style.space(12)
                    text: "›"
                    textFormat: Text.PlainText
                    color: root.dim
                    font.family: Style.font.family
                    font.pixelSize: Style.font.body
                  }
                }
              }
            }
          }

          Rectangle {
            width: root.accountsColumnOpen ? 1 : 0
            height: parent.height
            visible: root.accountsColumnOpen
            color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
          }

          Rectangle {
            width: parent.width - Style.space(281) - (root.accountsColumnOpen ? Style.space(261) : 0)
            height: parent.height
            color: root.background

            ContactDetail {
              anchors.fill: parent
              visible: !root.editing && !root.settingsOpen && !!root.current
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
              visible: !root.editing && !root.settingsOpen && !root.current
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

            EmptyState {
              anchors.fill: parent
              visible: root.settingsOpen && !root.editing && root.selectedOption.length === 0
              textColor: root.foreground
              dimColor: root.dim
              title: "Choose an option"
              body: "Open Accounts to connect iCloud."
            }

            EmptyState {
              anchors.fill: parent
              visible: root.settingsOpen && !root.editing && root.accountsColumnOpen && !root.icloudPaneOpen
              textColor: root.foreground
              dimColor: root.dim
              title: "Choose an account"
              body: "iCloud keeps your contacts in sync."
            }

            Column {
              visible: root.icloudPaneOpen && !root.editing
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
                text: "Paste the app-specific password from appleid.apple.com. The four groups with dashes are fine."
                textFormat: Text.PlainText
                color: root.dim
                font.family: Style.font.family
                font.pixelSize: Style.font.body
              }
              Text { text: "Apple ID"; textFormat: Text.PlainText; color: root.dim; font.family: Style.font.family; font.pixelSize: Style.font.body }
              TextField {
                id: appleIdField
                width: Math.min(parent.width, Style.space(360))
                enabled: !root.setupBusy
                onAccepted: passwordField.forceActiveFocus()
              }
              Text { text: "App-specific password"; textFormat: Text.PlainText; color: root.dim; font.family: Style.font.family; font.pixelSize: Style.font.body }
              TextField {
                id: passwordField
                width: appleIdField.width
                password: true
                enabled: !root.setupBusy
                onAccepted: root.saveIcloud()
              }
              Text {
                width: parent.width
                visible: root.setupError.length > 0
                wrapMode: Text.Wrap
                text: root.setupError
                textFormat: Text.PlainText
                color: Color.urgent
                font.family: Style.font.family
                font.pixelSize: Style.font.body
              }
              Row {
                spacing: Style.space(8)
                Button {
                  text: root.setupBusy ? "Working…" : "Save and sync"
                  enabled: !root.setupBusy
                  onClicked: root.saveIcloud()
                }
                Button {
                  text: "Sync now"
                  visible: !!(root.sync && (root.sync.has_password || root.sync.apple_id))
                  enabled: !root.setupBusy
                  onClicked: {
                    if (!root.svc) return
                    root.setupError = "Looking for your contacts on iCloud…"
                    root.setupBusy = true
                    root.svc.syncNow(function(syncRes) {
                      root.setupBusy = false
                      if (syncRes && syncRes.ok) root.setupError = ""
                      else root.setupError = root.svc.plain(syncRes && syncRes.error ? syncRes.error : "Sync did not finish")
                    })
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
