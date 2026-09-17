import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons

// Long-lived host. The window and the bar widget both read contact count
// and send commands through this object. The helper process is started
// when the shell loads the plugin, and started again on the next command
// if it has died.
Item {
  id: root
  visible: false
  width: 0
  height: 0

  property var shell: null
  property var manifest: null
  property var pluginRegistry: null
  property var barWidgetRegistry: null

  readonly property string pluginId: manifest && manifest.id ? String(manifest.id) : "omarchy-contacts"
  readonly property string pluginDir: decodeURIComponent(String(Qt.resolvedUrl("."))
    .replace(/^file:\/\//, "")).replace(/\/$/, "")
  readonly property string pluginName: manifest && manifest.name ? String(manifest.name) : "Contacts"
  readonly property string version: manifest && manifest.version ? String(manifest.version) : ""

  property var settings: ({
    syncIntervalSec: 300,
    showBarCount: false,
    watchImport: true
  })

  property int contactCount: 0
  property var contacts: []
  property var current: null
  property string query: ""
  property string notice: ""
  property string errorText: ""
  property bool daemonReady: false
  property bool daemonMissing: false
  property string daemonPath: ""
  property var sync: ({ enabled: false, interval_secs: 300, apple_id: "", has_password: false, syncing: false, progress: ({ phase: "", done: 0, total: 0 }) })
  property var keys: ({})
  property bool importing: false

  property int nextId: 1
  property var pending: ({})
  property var outbound: []

  function applySettings(next) {
    if (!next) return
    settings = next
    var interval = Number(next.syncIntervalSec)
    if (!isFinite(interval)) interval = 300
    if (interval < 120) interval = 120
    send("set_config", {
      interval_secs: interval,
      watch: next.watchImport !== false
    }, function() {})
  }

  function daemonCandidates() {
    return [
      root.pluginDir + "/omarchy-contactsd",
      root.pluginDir + "/target/release/omarchy-contactsd",
      root.pluginDir + "/target/debug/omarchy-contactsd"
    ]
  }

  function resolveDaemon() {
    // The helper is a sibling of this file after `make daemon`.
    var list = daemonCandidates()
    root.daemonPath = list[0]
    root.daemonMissing = false
    return list[0]
  }

  function ensureRunning() {
    if (proc.running) return
    proc.command = [root.resolveDaemon()]
    proc.running = true
  }

  function send(cmd, fields, cb) {
    root.ensureRunning()
    var id = root.nextId++
    var msg = fields ? Object.assign({ id: id, cmd: cmd }, fields) : { id: id, cmd: cmd }
    root.pending[id] = cb || function() {}
    root.outbound.push(msg)
    root.flush()
  }

  function flush() {
    if (!proc.running || !proc.stdinEnabled) return
    while (root.outbound.length > 0) {
      var msg = root.outbound.shift()
      var line = JSON.stringify(redact(msg))
      if (line.length > 1500000) {
        var cb = root.pending[msg.id]
        delete root.pending[msg.id]
        if (cb) cb({ ok: false, error: "request too large" })
        continue
      }
      proc.write(JSON.stringify(msg) + "\n")
    }
  }

  function redact(msg) {
    var copy = Object.assign({}, msg)
    if (copy.password) copy.password = "***"
    return copy
  }

  function handleLine(line) {
    var text = String(line || "")
    if (text.length > 2000000) return
    var obj = null
    try { obj = JSON.parse(text) } catch (e) { return }
    if (!obj) return
    if (obj.count !== undefined) root.contactCount = Number(obj.count) || 0
    if (obj.contacts) root.contacts = obj.contacts
    if (obj.contact) root.current = obj.contact
    if (obj.sync) root.sync = obj.sync
    if (obj.keys) root.keys = obj.keys
    if (obj.progress) {
      var s = Object.assign({}, root.sync)
      s.progress = obj.progress
      s.syncing = obj.progress && obj.progress.phase && obj.progress.phase !== "idle"
      root.sync = s
    }
    if (obj.ok === false && obj.error) root.errorText = plain(obj.error)
    var id = obj.id
    if (id !== undefined && root.pending[id]) {
      var cb = root.pending[id]
      delete root.pending[id]
      cb(obj)
    }
  }

  function plain(s) {
    return String(s == null ? "" : s)
      .replace(/[&<>]/g, "")
      .replace(/[\u0000-\u001f\u007f]/g, "")
      .substring(0, 240)
  }

  function list(q) {
    root.query = String(q || "")
    send("list", { query: root.query }, function(res) {
      if (res && res.ok) root.contacts = res.contacts || []
    })
  }

  function get(uid, cb) {
    send("get", { uid: String(uid || "") }, function(res) {
      if (res && res.ok) root.current = res.contact
      if (cb) cb(res)
    })
  }

  function save(contact, cb) {
    send("save", { contact: contact }, function(res) {
      if (res && res.ok) {
        root.current = res.contact
        root.list(root.query)
      }
      if (cb) cb(res)
    })
  }

  function remove(uid, cb) {
    send("delete", { uid: String(uid || "") }, function(res) {
      root.current = null
      root.list(root.query)
      if (cb) cb(res)
    })
  }

  function importFile(path, csv, cb) {
    root.importing = true
    send(csv ? "import_csv" : "import_vcf", { path: String(path || "") }, function(res) {
      root.importing = false
      if (res && res.ok) {
        root.notice = "Imported " + String(res.imported || 0) + " contacts"
        root.list(root.query)
      }
      if (cb) cb(res)
    })
  }

  function exportFile(path, uids, csv, cb) {
    var fields = { path: String(path || "") }
    if (uids && uids.length) fields.uids = uids
    send(csv ? "export_csv" : "export_vcf", fields, cb)
  }

  function emailCard(uid, cb) {
    send("email_card", { uid: String(uid || "") }, function(res) {
      if (cb) cb(res)
    })
  }

  function syncNow(cb) {
    send("sync_now", {}, function(res) {
      if (res && res.ok) root.errorText = ""
      root.list(root.query)
      if (cb) cb(res)
    })
  }

  function configureIcloud(appleId, password, cb) {
    send("set_icloud", { apple_id: String(appleId || ""), password: String(password || "") }, function(res) {
      if (res && res.ok) {
        root.errorText = ""
        send("status", {}, function() {})
      }
      if (cb) cb(res)
    })
  }

  function refreshStatus() {
    send("status", {}, function(res) {
      if (!res || !res.ok) return
      root.daemonReady = true
      if (res.keys) root.keys = res.keys
    })
  }

  Process {
    id: proc
    stdinEnabled: true
    stdout: SplitParser {
      splitMarker: "\n"
      onRead: function(data) { root.handleLine(data) }
    }
    stderr: SplitParser {
      splitMarker: "\n"
      onRead: function() {}
    }
    onStarted: {
      root.daemonReady = true
      root.daemonMissing = false
      root.flush()
      root.refreshStatus()
      root.list(root.query)
    }
    onExited: function(code) {
      root.daemonReady = false
      if (code === 127 || code === 1) {
        // Missing binary or failed to start: show the build hint once.
        root.daemonMissing = true
      }
    }
    onRunningChanged: if (running) Qt.callLater(root.flush)
  }

  Timer {
    interval: 5000
    running: true
    repeat: true
    onTriggered: {
      if (proc.running) root.send("tick", {}, function() {})
    }
  }

  Component.onCompleted: {
    root.ensureRunning()
  }
}
