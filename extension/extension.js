import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { makeDbusClient } from './dbusClient.js';
import { ChatPanelButton } from './panelMenu.js';

export default class ChatPeerExtension extends Extension {
  enable() {
    this._dbus = makeDbusClient({
      onPeerOnline: (peerId, username) => {
        if (this._panelBtn) this._panelBtn.onPeerOnline(peerId, username);
      },
      onPeerOffline: () => {},
      onMessageReceived: (peerId, username, content, msgId) => {
        if (this._panelBtn)
          this._panelBtn.onMessageReceived(peerId, username, content, msgId);
      },
    });

    this._panelBtn = new ChatPanelButton(this._dbus);
    Main.panel.addToStatusArea('chatpeer', this._panelBtn, 0, 'right');
  }

  disable() {
    if (this._panelBtn) {
      this._panelBtn.destroy();
      this._panelBtn = null;
    }
    this._dbus = null;
  }
}
