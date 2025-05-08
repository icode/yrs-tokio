# yrs-axum-ws

> Yrs message exchange protocol base on axum websocket

```rust
use yrs_axum_ws::{YrsSink, YrsStream};

use axum::extract::ws::WebSocket;
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use futures_util::StreamExt;
use net::TcpListener;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::{net, spawn};
use yrs::sync::Awareness;
use yrs::Doc;
use yrs_tokio::broadcast::BroadcastGroup;

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

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(bcast): State<Arc<BroadcastGroup>>,
) -> Response {
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