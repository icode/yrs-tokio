# yrs-tokio

> Yrs message exchange protocol base on tokio

This library is an extension over [Yjs](https://yjs.dev)/[Yrs](https://github.com/y-crdt/y-crdt) Conflict-Free
Replicated Data Types (CRDT) message exchange protocol,
and it does not have communication protocol restrictions.
It provides an utilities connect with Yjs provider using Rust tokio.
And it can support almost all tokio based frameworks,
e.g., [tokio-tungstenite](/yrs-tokio-tungstenite), [axum](/yrs-axum-ws), [warp](/yrs-warp-ws), [Rocket](/yrs-rocket-ws)
and so on.

### Examples

In order to gossip updates between different web socket connections from clients collaborating over the same logical
document, a broadcast group can be used. See examples:

- [tokio-tungstenite](/yrs-tokio-tungstenite/examples)
- [axum](/yrs-axum-ws/examples)
- [warp](/yrs-warp-ws/examples)
- [Rocket](/yrs-rocket-ws/examples)

### Custom framework example

You can use frameworks based on tokio that are not yet supported, just like the following:

```rust
use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::Sink;
use yrs_tokio::signaling::Message as SignalingMessage;
use yrs_tokio::{impl_yrs_signal_stream, to_signaling_message, yrs_common_sink, YrsExchange, YrsSink, YrsStream};

#[derive(YrsStream)]
pub struct YrsStream(SplitStream<WebSocket>);
#[derive(YrsExchange)]
pub struct YrsSignalStream(SplitStream<WebSocket>);

impl_yrs_signal_stream!(YrsSignalStream, item => to_signaling_message!(item));

#[derive(YrsSink)]
pub struct YrsSink(SplitSink<WebSocket, Message>);
#[yrs_common_sink]
impl Sink<SignalingMessage> for YrsSink {}

#[tokio::main]
async fn main() {
    // We're using a single static document shared among all the peers.
    let awareness = Arc::new(RwLock::new(Awareness::new(Doc::new())));

    // open a broadcast group that listens to awareness and document updates
    // and has a pending message buffer of up to 32 updates
    let bcast = Arc::new(BroadcastGroup::new(awareness, 32).await);

    let addr = SocketAddr::from_str("0.0.0.0:8080").unwrap();

    let app = Router::new()
        .route("/my-room", any(ws_handler))
        .with_state(bcast);

    spawn(async move {
        let listener = TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });
}

async fn ws_handler(ws: WebSocketUpgrade, State(bcast): State<Arc<BroadcastGroup>>) -> Response {
    ws.on_upgrade(move |socket| peer(socket, bcast))
}

async fn peer(ws: WebSocket, bcast: Arc<BroadcastGroup>) {
    let (sink, stream) = ws.split();
    let sink = Arc::new(Mutex::new(YrsSink::from(sink)));
    let stream = YrsStream::from(stream);

    let sub = bcast.subscribe(sink, stream);
    match sub.completed().await {
        Ok(_) => println!("broadcasting for channel finished successfully"),
        Err(e) => eprintln!("broadcasting for channel finished abruptly: {}", e),
    }
}
```

## Custom protocol extensions

[y-sync](https://crates.io/crates/y-sync) protocol enables to extend it's own protocol, and yrs-tokio supports this as
well.
This can be done by implementing your own protocol.

## y-webrtc and signaling service

Additionally to performing it's role as a [y-websocket](https://docs.yjs.dev/ecosystem/connection-provider/y-websocket)
server, tokio also provides a signaling server implementation used by [y-webrtc](https://github.com/yjs/y-webrtc)
clients to exchange information necessary to connect WebRTC peers together and make them subscribe/unsubscribe from
specific rooms.

## Thanks

`yrs-tokio` fork from [yrs-warp](https://github.com/y-crdt/yrs-warp)