import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import St from 'gi://St';

export class ChatView extends PopupMenu.PopupMenuSection {
  constructor(sourceActor, peerId, username, sendMessageFn, loadHistoryFn) {
    super();
    this._peerId = peerId;
    this._username = username;
    this._sendMessageFn = sendMessageFn;
    this._loadHistoryFn = loadHistoryFn;
    this._messages = [];
    this._closeHandler = null;

    const header = new St.BoxLayout({ style_class: 'chat-header' });
    const backBtn = new St.Button({
      style_class: 'chat-back-btn',
      label: '←',
    });
    backBtn.connect('clicked', () => {
      if (this._closeHandler) this._closeHandler();
    });
    const title = new St.Label({
      text: `Chat with ${username}`,
      style_class: 'chat-title',
    });
    header.add_child(backBtn);
    header.add_child(title);
    this.actor.add_child(header);

    this._scrollView = new St.ScrollView({
      style_class: 'chat-messages',
      reactive: true,
    });
    this._messageBox = new St.BoxLayout({
      vertical: true,
      style_class: 'chat-message-box',
    });
    this._scrollView.add_actor(this._messageBox);
    this.actor.add_child(this._scrollView);

    const inputBox = new St.BoxLayout({ style_class: 'chat-input-box' });
    this._entry = new St.Entry({
      style_class: 'chat-input',
      hint_text: 'Type a message...',
      can_focus: true,
    });
    const sendBtn = new St.Button({
      style_class: 'chat-send-btn',
      label: 'Send',
    });
    sendBtn.connect('clicked', () => this._send());
    this._entry.clutter_text.connect('activate', () => this._send());
    inputBox.add_child(this._entry);
    inputBox.add_child(sendBtn);
    this.actor.add_child(inputBox);

    this._loadHistory();
  }

  setCloseHandler(handler) {
    this._closeHandler = handler;
  }

  async _loadHistory() {
    if (!this._loadHistoryFn) return;
    try {
      const msgs = await this._loadHistoryFn(this._peerId, 50);
      for (const [_id, _peerId, isOutgoing, content, _ts, _delivered] of msgs) {
        const sender = isOutgoing ? 'You' : this._username;
        this._addMessage(sender, content, isOutgoing);
      }
    } catch (err) {
      logError(err, 'chatpeer: load history');
    }
  }

  _send() {
    const text = this._entry.text.trim();
    if (!text) return;
    this._sendMessageFn(this._peerId, text);
    this._addMessage('You', text, true);
    this._entry.text = '';
  }

  _addMessage(sender, text, isOwn) {
    const msg = new St.Label({
      text: `${sender}: ${text}`,
      style_class: isOwn ? 'chat-msg-own' : 'chat-msg-other',
    });
    this._messageBox.add_child(msg);
    this._scrollView.vscroll.adjustment.value =
      this._scrollView.vscroll.adjustment.upper;
  }

  receiveMessage(username, text) {
    this._addMessage(username, text, false);
  }
}
