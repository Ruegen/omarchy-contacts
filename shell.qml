//@ pragma AppId omarchy-contacts
//@ pragma ShellId omarchy-contacts
//@ pragma NativeTextRendering

import Quickshell
import QtQuick

// Standalone Contacts window. Launch from the app menu, same as Flea or Omacalc.
ShellRoot {
  Service {
    id: contactsService
  }

  App {
    id: app
    service: contactsService
    opened: true

    Component.onCompleted: app.open()
  }

  Connections {
    target: Quickshell
    function onLastWindowClosed() { Qt.quit() }
  }
}
