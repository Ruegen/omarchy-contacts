import QtQuick
import QtQuick.Controls
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
  property var selected: null
  property string searchText: ""
  property string groupFilter: ""
  property bool namingGroup: false
  property string newGroupDraft: ""
  property string pane: "list"
  property int groupCursor: 0
  property bool myCardActive: false
  property string pendingMyUid: ""
  readonly property string passwordMask: "••••••••••••••••"

  readonly property var allContacts: svc && svc.contacts ? svc.contacts : []
  readonly property var groupNames: collectGroupNames(allContacts)
  readonly property var groupItems: ["All"].concat(groupNames)
  property var contacts: []
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

  function rowAsDetail(row) {
    if (!row) return null
    var phone = String(row.phone || "")
    var email = String(row.email || "")
    var d = Object.assign({}, row)
    d.phones = Array.isArray(row.phones) ? row.phones : (phone ? [{ type: "other", value: phone }] : [])
    d.emails = Array.isArray(row.emails) ? row.emails : (email ? [{ type: "other", value: email }] : [])
    d.addresses = Array.isArray(row.addresses) ? row.addresses : []
    d.urls = Array.isArray(row.urls) ? row.urls : []
    d.ims = Array.isArray(row.ims) ? row.ims : []
    d.socials = Array.isArray(row.socials) ? row.socials : []
    d.related = Array.isArray(row.related) ? row.related : []
    d.dates = Array.isArray(row.dates) ? row.dates : []
    d.groups = Array.isArray(row.groups) ? row.groups : []
    return d
  }

  function movePane(dx) {
    var order = ["groups", "list", "detail"]
    var i = order.indexOf(root.pane)
    if (i < 0) i = 1
    root.pane = order[Math.max(0, Math.min(order.length - 1, i + dx))]
    Qt.callLater(function() { if (focusScope) focusScope.forceActiveFocus() })
  }

  function appleId() {
    return root.sync && root.sync.apple_id ? String(root.sync.apple_id).trim() : ""
  }

  function emailsOf(row) {
    var out = []
    if (!row) return out
    if (row.email) out.push(String(row.email))
    var emails = row.emails || []
    for (var i = 0; i < emails.length; i++) {
      var v = emails[i] && emails[i].value ? String(emails[i].value) : ""
      if (v) out.push(v)
    }
    return out
  }

  function findMyCard(list, appleId) {
    var needle = String(appleId || "").trim().toLowerCase()
    if (!needle || !list || !list.length) return null
    var local = needle.split("@")[0]
    var localHit = null
    for (var i = 0; i < list.length; i++) {
      var c = list[i]
      if (!c || c.is_group) continue
      var emails = root.emailsOf(c)
      for (var j = 0; j < emails.length; j++) {
        var e = emails[j].trim().toLowerCase()
        if (e === needle) return c
        if (!localHit && local && e.split("@")[0] === local) localHit = c
      }
    }
    return localHit
  }

  function selectUid(uid) {
    var want = root.uidKey(uid)
    if (!want) return false
    for (var i = 0; i < root.contacts.length; i++) {
      if (root.uidKey(root.contacts[i].uid) === want) {
        if (root.cursor !== i) root.cursor = i
        if (!root.selected || root.uidKey(root.selected.uid) !== want)
          root.selectCurrent()
        return true
      }
    }
    return false
  }

  function openMyCard() {
    var alreadyAll = !String(root.groupFilter || "")
    root.myCardActive = true
    if (!alreadyAll) {
      root.groupFilter = ""
      root.groupCursor = 0
    }
    var card = root.findMyCard(root.allContacts, root.appleId())
    if (!card) {
      root.pendingMyUid = ""
      root.selected = null
      return
    }
    if (alreadyAll && root.selectUid(card.uid)) {
      root.pendingMyUid = ""
      return
    }
    root.pendingMyUid = String(card.uid || "")
    root.selected = root.rowAsDetail(card)
  }

  function applyGroupCursor() {
    root.myCardActive = false
    if (!root.groupItems.length) {
      root.groupFilter = ""
      return
    }
    var label = String(root.groupItems[root.groupCursor] || "All")
    root.groupFilter = label === "All" ? "" : label
    root.cursor = 0
    root.selectCurrent()
  }

  function moveGroup(dy) {
    if (root.myCardActive) {
      if (dy > 0) {
        root.myCardActive = false
        root.groupCursor = 0
        root.applyGroupCursor()
      }
      return
    }
    if (dy < 0 && root.groupCursor <= 0) {
      root.openMyCard()
      return
    }
    if (!root.groupItems.length) return
    root.groupCursor = Math.max(0, Math.min(root.groupItems.length - 1, root.groupCursor + dy))
    root.applyGroupCursor()
  }

  function syncGroupCursor() {
    if (root.myCardActive) return
    if (!root.groupFilter) {
      root.groupCursor = 0
      return
    }
    for (var i = 0; i < root.groupItems.length; i++) {
      if (String(root.groupItems[i]) === root.groupFilter) {
        root.groupCursor = i
        return
      }
    }
    root.groupCursor = 0
    root.groupFilter = ""
  }

  function chooseGroup(index) {
    root.groupCursor = index
    root.pane = "groups"
    root.applyGroupCursor()
  }

  function selectCurrent() {
    if (!contacts.length || cursor < 0 || cursor >= contacts.length) {
      selected = null
      return
    }
    var row = contacts[cursor]
    var uid = String(row && row.uid ? row.uid : "")
    if (!selected || String(selected.uid || "") !== uid)
      selected = rowAsDetail(row)
    if (!uid || !svc || typeof svc.get !== "function") return
    svc.get(uid, function(res) {
      if (!res || !res.ok || !res.contact) return
      if (String(res.contact.uid || "") !== String(root.selectedUid())) return
      selected = rowAsDetail(res.contact)
    })
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
    if (!selected) selectCurrent()
    if (!selected) return
    editing = true
    draft = Object.assign({}, selected)
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

  function collectGroupNames(list) {
    var seen = ({})
    var out = []
    if (!list || !list.length) return out
    for (var i = 0; i < list.length; i++) {
      var c = list[i]
      if (c && c.is_group && c.fn) {
        var n = String(c.fn)
        if (n && !seen[n]) { seen[n] = true; out.push(n) }
      }
      var gs = c && c.groups ? c.groups : []
      for (var j = 0; j < gs.length; j++) {
        var g = String(gs[j] || "").trim()
        if (g && !seen[g]) { seen[g] = true; out.push(g) }
      }
    }
    out.sort()
    return out
  }

  function uidKey(uid) {
    return String(uid || "").toLowerCase().replace(/urn:uuid:/g, "").replace(/[^a-f0-9]/g, "")
  }

  function contactInGroup(c, name, list) {
    if (!c || !name) return true
    var gs = c.groups || []
    for (var i = 0; i < gs.length; i++) {
      if (String(gs[i]) === name) return true
    }
    var key = uidKey(c.uid)
    for (var j = 0; j < list.length; j++) {
      var g = list[j]
      if (!g || !g.is_group || String(g.fn) !== name) continue
      var ms = g.members || []
      for (var k = 0; k < ms.length; k++) {
        var mk = uidKey(ms[k])
        if (mk && key && (mk === key || key.indexOf(mk) >= 0 || mk.indexOf(key) >= 0)) return true
      }
    }
    return false
  }

  function contactsUnchanged(a, b) {
    if (a === b) return true
    if (!a || !b || a.length !== b.length) return false
    for (var i = 0; i < a.length; i++) {
      if (String(a[i] && a[i].uid || "") !== String(b[i] && b[i].uid || ""))
        return false
    }
    return true
  }

  function rebuildContacts() {
    var next = filterContacts(allContacts, searchText, groupFilter)
    if (contactsUnchanged(contacts, next)) return
    contacts = next
  }

  function filterContacts(list, q, group) {
    if (!list || !list.length) return []
    var needle = String(q || "").trim().toLowerCase()
    var groupName = String(group || "")
    var out = []
    for (var i = 0; i < list.length; i++) {
      var c = list[i]
      if (c && c.is_group) continue
      if (groupName && !contactInGroup(c, groupName, list)) continue
      if (!needle) {
        out.push(c)
        continue
      }
      var hay = [
        c.fn, c.first, c.last, c.middle, c.nickname, c.org, c.department, c.title, c.role, c.phone, c.email, c.note, c.bday
      ]
      var hit = false
      for (var j = 0; j < hay.length; j++) {
        if (String(hay[j] || "").toLowerCase().indexOf(needle) !== -1) {
          hit = true
          break
        }
      }
      if (hit) out.push(c)
    }
    return out
  }

  function applySearch() {
    cursor = 0
    if (contacts.length) selectCurrent()
    else selected = null
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
    picker.uids = []
    picker.command = ["/usr/bin/zenity", "--file-selection", "--save", "--confirm-overwrite",
      "--filename=" + (csv ? "contacts.csv" : "contacts.vcf")]
    picker.running = true
  }

  function pickExportSelected() {
    var uid = selectedUid()
    if (!uid) return
    var raw = selected && selected.fn ? String(selected.fn) : "contact"
    var name = raw.replace(/[^A-Za-z0-9]+/g, "-").replace(/^-|-$/g, "")
    if (!name) name = "contact"
    picker.csv = false
    picker.save = true
    picker.uids = [uid]
    picker.command = ["/usr/bin/zenity", "--file-selection", "--save", "--confirm-overwrite",
      "--filename=" + name + ".vcf"]
    picker.running = true
  }

  function emailSelected() {
    var uid = selectedUid()
    if (!uid || !svc || typeof svc.emailCard !== "function") return
    svc.emailCard(uid, function(res) {
      if (res && res.ok) return
      if (svc) svc.errorText = svc.plain(res && res.error ? res.error : "Could not open mail with this card")
    })
  }

  function startNewGroup() {
    namingGroup = true
    newGroupDraft = ""
    myCardActive = false
    pane = "groups"
    Qt.callLater(function() {
      if (!groupList) return
      groupList.takeNameFocus()
    })
  }

  function createGroup() {
    if (groupList) newGroupDraft = groupList.nameText()
    var name = String(newGroupDraft || "").trim()
    if (!name || !svc) return
    var existing = groupNames
    for (var i = 0; i < existing.length; i++) {
      if (String(existing[i]).toLowerCase() === name.toLowerCase()) {
        groupFilter = existing[i]
        namingGroup = false
        newGroupDraft = ""
        cursor = 0
        selectCurrent()
        return
      }
    }
    var uid = selectedUid()
    var members = uid ? [uid] : []
    svc.save({
      fn: name,
      first: "",
      last: "",
      is_group: true,
      members: members,
      phones: [],
      emails: []
    }, function(res) {
      if (!res || !res.ok) return
      function finish() {
        namingGroup = false
        newGroupDraft = ""
        groupFilter = name
        cursor = 0
        if (svc) svc.list("")
        Qt.callLater(root.selectCurrent)
      }
      if (uid && selected) {
        var gs = []
        var cur = selected.groups || []
        for (var j = 0; j < cur.length; j++) gs.push(String(cur[j]))
        if (gs.indexOf(name) < 0) gs.push(name)
        var person = Object.assign({}, selected)
        person.groups = gs
        svc.save(person, function() { finish() })
      } else {
        finish()
      }
    })
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
      if (namingGroup) {
        namingGroup = false
        newGroupDraft = ""
        event.accepted = true
        return
      }
      if (searchFocused) {
        searchFocused = false
        searchText = ""
        if (searchField && searchField.text) searchField.text = ""
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
      pane = "list"
      moveCursor(event.key === Qt.Key_Down ? 1 : -1)
      event.accepted = true
      return
    }
    if (searchFocused || namingGroup) return
    if (Keymap.matchChord(event, keys.left) || Keymap.matchChord(event, keys.leftAlt)) {
      movePane(-1); event.accepted = true; return
    }
    if (Keymap.matchChord(event, keys.right) || Keymap.matchChord(event, keys.rightAlt)) {
      movePane(1); event.accepted = true; return
    }
    if (event.key === Qt.Key_Tab) {
      movePane((event.modifiers & Qt.ShiftModifier) ? -1 : 1)
      event.accepted = true
      return
    }
    if (Keymap.matchChord(event, keys.down) || Keymap.matchChord(event, keys.downAlt)) {
      if (pane === "groups") moveGroup(1)
      else moveCursor(1)
      event.accepted = true
      return
    }
    if (Keymap.matchChord(event, keys.up) || Keymap.matchChord(event, keys.upAlt)) {
      if (pane === "groups") moveGroup(-1)
      else moveCursor(-1)
      event.accepted = true
      return
    }
    if (Keymap.matchChord(event, keys.open)) {
      if (pane === "detail") startEdit()
      else if (pane === "groups") {
        if (myCardActive) openMyCard()
        else applyGroupCursor()
      }
      else selectCurrent()
      event.accepted = true
      return
    }
    if (Keymap.matchChord(event, keys.newContact)) { startNew(); event.accepted = true; return }
    if (Keymap.matchChord(event, keys.newGroup)) { startNewGroup(); event.accepted = true; return }
    if (Keymap.matchChord(event, keys.edit)) { startEdit(); event.accepted = true; return }
    if (Keymap.matchChord(event, keys.deleteContact)) { confirmDelete = true; event.accepted = true; return }
    if (Keymap.matchChord(event, keys.mail)) { emailSelected(); event.accepted = true; return }
    if (Keymap.matchChord(event, keys.sync)) {
      if (svc) svc.syncNow(function() {})
      event.accepted = true
      return
    }
    if (Keymap.matchChord(event, keys.importFile)) { pickImport(false); event.accepted = true; return }
    if (Keymap.matchChord(event, keys.exportFile)) { pickExport(false); event.accepted = true; return }
  }

  onAllContactsChanged: rebuildContacts()
  onSearchTextChanged: rebuildContacts()
  onGroupFilterChanged: rebuildContacts()
  Component.onCompleted: rebuildContacts()

  onContactsChanged: {
    if (root.pendingMyUid) {
      if (root.contacts.length && root.selectUid(root.pendingMyUid))
        root.pendingMyUid = ""
      return
    }
    if (root.myCardActive) return
    if (cursor >= contacts.length) cursor = Math.max(0, contacts.length - 1)
    if (!contacts.length) {
      selected = null
      return
    }
    if (selected && root.uidKey(selected.uid) === root.uidKey(selectedUid())) return
    selectCurrent()
  }

  onGroupNamesChanged: syncGroupCursor()

  Process {
    id: picker
    property bool csv: false
    property bool save: false
    property var uids: []
    stdout: SplitParser {
      splitMarker: "\n"
      onRead: function(data) {
        var path = String(data || "").replace(/\r/g, "").trim()
        if (!path || path.length > 4096) return
        if (picker.save) {
          if (root.svc) root.svc.exportFile(path, picker.uids, picker.csv, function() {})
          picker.uids = []
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
    implicitWidth: Style.space(1040)
    implicitHeight: Style.space(640)
    minimumSize: Qt.size(Style.space(840), Style.space(500))

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
            id: headerLeft
            anchors.left: parent.left
            anchors.leftMargin: Style.space(14)
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(8)

            Button {
              anchors.verticalCenter: parent.verticalCenter
              iconText: "☰"
              tooltipText: "Options"
              foreground: root.dim
              accent: root.accent
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
              width: Style.space(220)
              placeholderText: "Search"
              onActiveFocusChanged: root.searchFocused = activeFocus
              onTextChanged: {
                if (root.searchText === text) return
                root.searchText = text
                root.applySearch()
              }
            }
          }

          Row {
            id: headerRight
            anchors.right: parent.right
            anchors.rightMargin: Style.space(14)
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(4)

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
                    if (total > 0) {
                    if (phase === "photos") return "Photos " + done + "/" + total
                    if (phase === "upload") return "Sending " + done + "/" + total
                    return "Syncing " + done + "/" + total
                  }
                  if (phase === "discover" || phase === "listing") return "Looking up iCloud…"
                  return "Syncing…"
                }
                textFormat: Text.PlainText
                color: root.accent
                font.family: Style.font.family
                font.pixelSize: Style.font.caption
              }
            }

            Text {
              anchors.verticalCenter: parent.verticalCenter
              visible: root.notice.length > 0 && !root.syncing
              width: Math.min(implicitWidth, Style.space(160))
              text: root.svc ? root.svc.plain(root.notice) : ""
              textFormat: Text.PlainText
              color: root.dim
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
            }

            Button {
              visible: !root.settingsOpen && !root.editing
              text: "New contact"
              tooltipText: "New contact · n"
              bordered: true
              foreground: root.foreground
              accent: root.accent
              onClicked: root.startNew()
            }
            Button {
              visible: !root.settingsOpen && !root.editing
              text: "Edit"
              tooltipText: "Edit · e"
              bordered: true
              foreground: root.foreground
              accent: root.accent
              enabled: !!root.selected
              onClicked: root.startEdit()
            }
            Button {
              visible: !root.settingsOpen && !root.editing
              text: "Delete"
              tooltipText: "Delete · d"
              bordered: true
              foreground: root.foreground
              accent: root.accent
              enabled: !!root.selected
              onClicked: root.confirmDelete = true
            }
            Rectangle {
              visible: !root.settingsOpen && !root.editing
              anchors.verticalCenter: parent.verticalCenter
              width: 1
              height: Style.space(22)
              color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.16)
            }
            Button {
              visible: !root.settingsOpen && !root.editing
              text: "Email card"
              tooltipText: "Attach this card to a new email · m"
              bordered: true
              foreground: root.foreground
              accent: root.accent
              enabled: !!root.selected
              onClicked: root.emailSelected()
            }
            Button {
              visible: !root.settingsOpen && !root.editing
              text: "Export all"
              tooltipText: "Save every contact in one vCard file · Ctrl+E"
              bordered: true
              foreground: root.foreground
              accent: root.accent
              enabled: root.allContacts.length > 0
              onClicked: root.pickExport(false)
            }
            Button {
              visible: root.editing
              text: "Save"
              tooltipText: "Save · Ctrl+S"
              bordered: true
              foreground: root.foreground
              accent: root.accent
              onClicked: root.saveDraft()
            }
            Button {
              visible: root.editing
              text: "Cancel"
              tooltipText: "Cancel · Esc"
              bordered: true
              foreground: root.foreground
              accent: root.accent
              onClicked: root.editing = false
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
          height: parent.height - Style.space(52) - 1 - Style.space(36) - 1

          Rectangle {
            width: Style.space(200)
            height: parent.height
            color: root.background
            clip: true

            GroupList {
              id: groupList
              anchors.fill: parent
              visible: !root.settingsOpen
              groups: root.groupItems
              cursor: root.groupCursor
              focused: root.pane === "groups" && !root.settingsOpen
              naming: root.namingGroup
              myCardActive: root.myCardActive
              textColor: root.foreground
              dimColor: root.dim
              accent: root.accent
              background: root.background
              onActivated: function(index) { root.chooseGroup(index) }
              onMyCardRequested: root.openMyCard()
              onAddRequested: root.startNewGroup()
              onNameAccepted: root.createGroup()
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
            width: root.settingsOpen ? (root.accountsColumnOpen ? Style.space(260) : 0) : Style.space(280)
            height: parent.height
            visible: !root.settingsOpen || root.accountsColumnOpen
            color: root.background
            clip: true

            ContactList {
              id: list
              anchors.fill: parent
              anchors.margins: Style.space(6)
              visible: !root.settingsOpen && root.contacts.length > 0 && !(root.svc && root.svc.daemonMissing)
              contacts: root.contacts
              cursor: root.cursor
              focused: root.pane === "list" && !root.settingsOpen
              textColor: root.foreground
              dimColor: root.dim
              accent: root.accent
              background: root.background
              onActivated: function(index) {
                root.pane = "list"
                root.myCardActive = false
                root.cursor = index
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

            Column {
              visible: root.accountsColumnOpen
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
            width: (!root.settingsOpen || root.accountsColumnOpen) ? 1 : 0
            height: parent.height
            visible: !root.settingsOpen || root.accountsColumnOpen
            color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
          }

          Rectangle {
            width: parent.width - Style.space(201) - ((!root.settingsOpen || root.accountsColumnOpen) ? (root.settingsOpen ? Style.space(261) : Style.space(281)) : 0)
            height: parent.height
            color: root.background

            ContactDetail {
              anchors.fill: parent
              visible: !root.editing && !root.settingsOpen && !!root.selected
              contact: root.selected
              textColor: root.foreground
              dimColor: root.dim
              accent: root.accent
              background: root.background
            }

            EmptyState {
              anchors.fill: parent
              visible: !root.editing && !root.settingsOpen && !root.selected
              textColor: root.foreground
              dimColor: root.dim
              title: root.myCardActive ? "No card for you yet" : "Select a contact"
              body: root.myCardActive
                ? "Your card is the contact that uses your iCloud email."
                : "h/l or Tab for columns. j/k to move. Enter opens. e edits."
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
                  bordered: true
                  foreground: root.foreground
                  accent: root.accent
                  onClicked: root.saveIcloud()
                }
                Button {
                  text: "Sync now"
                  visible: !!(root.sync && (root.sync.has_password || root.sync.apple_id))
                  enabled: !root.setupBusy
                  bordered: true
                  foreground: root.foreground
                  accent: root.accent
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
                  Button { text: "Delete"; bordered: true; foreground: root.foreground; accent: root.accent; onClicked: root.deleteSelected() }
                  Button { text: "Cancel"; bordered: true; foreground: root.foreground; accent: root.accent; onClicked: root.confirmDelete = false }
                }
              }
            }
          }
        }

        Rectangle {
          width: parent.width
          height: 1
          color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
        }

        Rectangle {
          width: parent.width
          height: Style.space(36)
          color: root.popupBackground

          Text {
            anchors.verticalCenter: parent.verticalCenter
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: Style.space(16)
            anchors.rightMargin: Style.space(16)
            text: {
              var g = root.pane === "groups" ? "[groups]" : "groups"
              var l = root.pane === "list" ? "[list]" : "list"
              var d = root.pane === "detail" ? "[card]" : "card"
              return g + "  " + l + "  " + d + "    h/l columns · j/k move · / search · n new · g group · e edit · d delete · m mail · ? keys"
            }
            textFormat: Text.PlainText
            color: root.dim
            elide: Text.ElideRight
            font.family: Style.font.family
            font.pixelSize: Style.font.body
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
