import Gio from 'gi://Gio';

const DBusIface = `
<node>
  <interface name="com.chatpeer.Daemon">
    <method name="SendMessage">
      <arg type="s" name="peer_id" direction="in"/>
      <arg type="s" name="content" direction="in"/>
      <arg type="s" name="msg_id" direction="out"/>
    </method>
    <method name="GetOnlinePeers">
      <arg type="a(sss)" name="peers" direction="out"/>
    </method>
    <method name="GetMyPeerId">
      <arg type="s" name="peer_id" direction="out"/>
    </method>
    <method name="GetConversation">
      <arg type="s" name="peer_id" direction="in"/>
      <arg type="u" name="limit" direction="in"/>
      <arg type="a(ssbsb)" name="messages" direction="out"/>
    </method>
    <method name="ListConversations">
      <arg type="as" name="peer_ids" direction="out"/>
    </method>
    <signal name="PeerOnline">
      <arg type="s" name="peer_id"/>
      <arg type="s" name="username"/>
    </signal>
    <signal name="MessageReceived">
      <arg type="s" name="peer_id"/>
      <arg type="s" name="username"/>
      <arg type="s" name="content"/>
      <arg type="s" name="msg_id"/>
    </signal>
  </interface>
</node>`;

export function makeDbusClient(callbacks) {
  const { onPeerOnline, onPeerOffline, onMessageReceived } = callbacks;
  const conn = Gio.DBus.session.get();

  const proxy = Gio.DBusProxy.makeProxyWrapper(DBusIface);
  const instance = new proxy(
    conn,
    'com.chatpeer.Daemon',
    '/com/chatpeer/Daemon'
  );

  // Connect to signals
  conn.signalSubscribe(
    'com.chatpeer.Daemon',
    'PeerOnline',
    '/com/chatpeer/Daemon',
    'com.chatpeer.Daemon',
    (_conn, _sender, _path, _iface, _signal, params) => {
      const [peerId, username] = params.deepUnpack();
      onPeerOnline(peerId, username);
    }
  );

  conn.signalSubscribe(
    'com.chatpeer.Daemon',
    'MessageReceived',
    '/com/chatpeer/Daemon',
    'com.chatpeer.Daemon',
    (_conn, _sender, _path, _iface, _signal, params) => {
      const [peerId, username, content, msgId] = params.deepUnpack();
      onMessageReceived(peerId, username, content, msgId);
    }
  );

  return {
    getOnlinePeers() {
      return new Promise((resolve, reject) => {
        instance.GetOnlinePeersRemote((result, err) => {
          if (err) reject(err);
          else resolve(result[0]);
        });
      });
    },
    sendMessage(peerId, content) {
      return new Promise((resolve, reject) => {
        instance.SendMessageRemote(peerId, content, (result, err) => {
          if (err) reject(err);
          else resolve(result[0]);
        });
      });
    },
    getMyPeerId() {
      return new Promise((resolve, reject) => {
        instance.GetMyPeerIdRemote((result, err) => {
          if (err) reject(err);
          else resolve(result[0]);
        });
      });
    },
    getConversation(peerId, limit = 50) {
      return new Promise((resolve, reject) => {
        instance.GetConversationRemote(peerId, limit, (result, err) => {
          if (err) reject(err);
          else resolve(result[0]);
        });
      });
    },
    listConversations() {
      return new Promise((resolve, reject) => {
        instance.ListConversationsRemote((result, err) => {
          if (err) reject(err);
          else resolve(result[0]);
        });
      });
    },
  };
}
