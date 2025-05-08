# yrs-warp-ws

> Yrs message exchange protocol base on warp websocket

```rust
use yrs_warp::{YrsStream, YrsSink};

use futures_util::{ready, SinkExt, StreamExt};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use tokio::task::JoinHandle;
use warp::{Filter, Rejection, Reply};
use warp::ws::{WebSocket, Ws};
use yrs::updates::encoder::Encode;
use yrs::{GetString, Text, Transact};
use yrs_tokio::broadcast::BroadcastGroup;

async fn start_server(
    addr: &str,
    bcast: Arc<BroadcastGroup>,
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    let addr = SocketAddr::from_str(addr)?;
    let ws = warp::path("my-room")
        .and(warp::ws())
        .and(warp::any().map(move || bcast.clone()))
        .and_then(ws_handler);

    Ok(tokio::spawn(async move {
        warp::serve(ws).run(addr).await;
    }))
}

async fn ws_handler(ws: Ws, bcast: Arc<BroadcastGroup>) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| peer(socket, bcast)))
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