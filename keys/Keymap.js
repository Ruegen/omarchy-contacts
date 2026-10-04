// Keyboard map. The overlay reads this table, so the sheet cannot drift
// from what the keys actually do. Bindings are user-overridable through
// the daemon config (`keys` in config.toml).
.pragma library

function defaults() {
  return ({
    search: "/",
    escape: "Esc",
    up: "Up",
    down: "Down",
    upAlt: "k",
    downAlt: "j",
    left: "Left",
    right: "Right",
    leftAlt: "h",
    rightAlt: "l",
    open: "Enter",
    newContact: "n",
    newGroup: "g",
    deleteContact: "d",
    edit: "e",
    mail: "m",
    sync: "s",
    importFile: "Ctrl+I",
    exportFile: "Ctrl+E",
    save: "Ctrl+S",
    help: "?",
    openPanel: "Super+C"
  })
}

function merge(over) {
  var d = defaults()
  if (!over) return d
  function take(k, alt) {
    var v = over[k]
    if (v === undefined && alt) v = over[alt]
    if (typeof v === "string" && v.length > 0) d[k] = v
  }
  take("search")
  take("escape")
  take("up")
  take("down")
  take("upAlt", "up_alt")
  take("downAlt", "down_alt")
  take("left")
  take("right")
  take("leftAlt", "left_alt")
  take("rightAlt", "right_alt")
  take("open")
  take("newContact", "new")
  take("newGroup", "new_group")
  take("deleteContact", "delete")
  take("edit")
  take("mail")
  take("sync")
  take("importFile", "import")
  take("exportFile", "export")
  take("save")
  take("help")
  take("openPanel", "open_panel")
  return d
}

function rows(bindings) {
  var b = merge(bindings)
  return [
    { group: "Window", items: [
      { keys: b.openPanel, action: "Open or close Contacts" },
      { keys: b.escape, action: "Close search, overlay, or window" },
      { keys: b.help, action: "Show this shortcut list" }
    ]},
    { group: "List", items: [
      { keys: b.search, action: "Focus search" },
      { keys: b.left + " / " + b.leftAlt, action: "Previous column" },
      { keys: b.right + " / " + b.rightAlt, action: "Next column" },
      { keys: "Tab", action: "Cycle columns" },
      { keys: b.up + " / " + b.upAlt, action: "Move up in this column" },
      { keys: b.down + " / " + b.downAlt, action: "Move down in this column" },
      { keys: b.open, action: "Open or edit" },
      { keys: b.newContact, action: "New contact" },
      { keys: b.newGroup, action: "New group" },
      { keys: b.edit, action: "Edit the selected contact" },
      { keys: b.deleteContact, action: "Delete the selected contact" },
      { keys: b.mail, action: "Mail this card" },
      { keys: b.sync, action: "Sync with iCloud now" }
    ]},
    { group: "Files", items: [
      { keys: b.importFile, action: "Import a vCard file" },
      { keys: b.exportFile, action: "Export a vCard file" }
    ]},
    { group: "Editing", items: [
      { keys: "Tab / Shift+Tab", action: "Move between fields" },
      { keys: b.save, action: "Save" },
      { keys: b.escape, action: "Cancel editing" }
    ]}
  ]
}

function matchChord(event, chord) {
  var s = String(chord || "")
  var wantCtrl = s.indexOf("Ctrl+") >= 0
  var wantShift = s.indexOf("Shift+") >= 0
  var wantAlt = s.indexOf("Alt+") >= 0
  var wantSuper = s.indexOf("Super+") >= 0
  var key = s.replace("Ctrl+", "").replace("Shift+", "").replace("Alt+", "").replace("Super+", "")
  if (!!event.modifiers && (event.modifiers & Qt.ControlModifier) ? true : false !== wantCtrl
      && key.length === 1) {
    // Compared below with explicit flags from the event.
  }
  var ctrl = !!(event.modifiers & Qt.ControlModifier)
  var shift = !!(event.modifiers & Qt.ShiftModifier)
  var alt = !!(event.modifiers & Qt.AltModifier)
  var superKey = !!(event.modifiers & Qt.MetaModifier)
  if (ctrl !== wantCtrl || alt !== wantAlt || superKey !== wantSuper) return false
  if (wantShift && !shift) return false
  if (!wantShift && shift && key.length === 1) {
    // Allow Shift for uppercase letters on help "?".
  }
  return keyMatches(event, key)
}

function keyMatches(event, key) {
  var k = String(key)
  if (k === "Esc" || k === "Escape") return event.key === Qt.Key_Escape
  if (k === "Enter" || k === "Return") return event.key === Qt.Key_Return || event.key === Qt.Key_Enter
  if (k === "Up") return event.key === Qt.Key_Up
  if (k === "Down") return event.key === Qt.Key_Down
  if (k === "Left") return event.key === Qt.Key_Left
  if (k === "Right") return event.key === Qt.Key_Right
  if (k === "Tab") return event.key === Qt.Key_Tab
  if (k === "/" ) return event.key === Qt.Key_Slash || (event.text === "/")
  if (k === "?") return event.text === "?" || (event.key === Qt.Key_Question)
  if (k.length === 1) {
    var ch = k.toLowerCase()
    if (event.text && event.text.toLowerCase() === ch) return true
    var code = ch.charCodeAt(0)
    if (code >= 97 && code <= 122) return event.key === (Qt.Key_A + (code - 97))
  }
  return false
}

function contextFor(state) {
  if (state.helpOpen) return "help"
  if (state.editing) return "edit"
  if (state.searchFocused) return "search"
  if (state.setupOpen) return "setup"
  return "list"
}
