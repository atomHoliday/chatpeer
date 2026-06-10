import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import St from 'gi://St';
import GObject from 'gi://GObject';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { ChatView } from './chatDialog.js';

export class ChatPanelButton extends PanelMenu.Button {
  constructor(dbusClient) {
    super(0.0, 'ChatPeer', false);

    this._dbus = dbusClient;
    this._conversations = {}; // peerId -> { username, chatView }
    this._onlinePeers = {}; // peerId -> username

    // Top bar icon
    const icon = new St.Icon({
      icon_name: 'user-available-symbolic',
      style_class: 'system-status-icon',
    });
    this.add_child(icon);

    this.menu.connect('open-state-changed', (menu, isOpen) => {
      if (isOpen) this._refresh();
    });

    // Wire up D-Bus callbacks
    this._dbus = dbusClient;
  }

  async _refresh() {
    this.menu.removeAll();

    // Online peers section
    const onlineSection = new PopupMenu.PopupMenuSection();
    const header = new PopupMenu.PopupMenuItem('Online', {
      reactive: false,
      can_focus: false,
    });
    header.actor.style = 'font-weight: bold; padding: 4px 12px;';
    onlineSection.addMenuItem(header);

    try {
      const peers = await this._dbus.getOnlinePeers();
      if (peers.length === 0) {
        const item = new PopupMenu.PopupMenuItem('No peers online');
        item.actor.reactive = false;
        onlineSection.addMenuItem(item);
      } else {
        for (const [username, peerId, status] of peers) {
          const item = new PopupMenu.PopupMenuItem(`● ${username}`);
          item.connect('activate', () => this._openChat(peerId, username));
          onlineSection.addMenuItem(item);
        }
      }
    } catch (err) {
      logError(err, 'chatpeer: failed to get online peers');
    }
    this.menu.addMenuItem(onlineSection);

    // Active chats section
    const chatIds = Object.keys(this._conversations);
    if (chatIds.length > 0) {
      this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
      const chatsHeader = new PopupMenu.PopupMenuItem('Active Chats', {
        reactive: false,
        can_focus: false,
      });
      chatsHeader.actor.style = 'font-weight: bold; padding: 4px 12px;';
      this.menu.addMenuItem(chatsHeader);
      for (const peerId of chatIds) {
        const conv = this._conversations[peerId];
        const item = new PopupMenu.PopupMenuItem(
          `💬 ${conv.username}`
        );
        item.connect('activate', () => this._openChat(peerId, conv.username));
        this.menu.addMenuItem(item);
      }
    }
  }

  _openChat(peerId, username) {
    this.menu.close();

    if (!this._conversations[peerId]) {
      const chatView = new ChatView(
        this.actor,
        peerId,
        username,
        (id, text) => this._dbus.sendMessage(id, text),
        (id, limit) => this._dbus.getConversation(id, limit)
      );
      chatView.connect('close', () => {
        delete this._conversations[peerId];
      });
      this._conversations[peerId] = { username, chatView };

      // Add to main menu
      this.menu.addMenuItem(chatView);
    }
  }

  onPeerOnline(peerId, username) {
    this._onlinePeers[peerId] = username;
  }

  onMessageReceived(peerId, username, content, msgId) {
    if (this._conversations[peerId]) {
      this._conversations[peerId].chatView.receiveMessage(username, content);
    }
    // Show notification if not in the chat view
    Main.notify(`ChatPeer: ${username}`, content);
  }
}

GObject.registerClass(ChatPanelButton);
