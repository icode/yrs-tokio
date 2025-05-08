# yrs-tokio-tungstenite

> Yrs message exchange protocol base on tokio-tungstenite

```rust
use yrs_tokio_tungstenite::{YrsStream, YrsSink};

use futures_util::{StreamExt};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, MaybeTlsStream, WebSocketStream};
use yrs_tokio::broadcast::BroadcastGroup;

async fn start_server(
    addr: &str,
    bcast: Arc<BroadcastGroup>,
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    let addr = SocketAddr::from_str(addr)?;

    let bcast_clone = bcast.clone();
    Ok(tokio::spawn(async move {
        let listener = TcpListener::bind(addr).await.unwrap();

        while let Ok((stream, _)) = listener.accept().await {
            let bcast = bcast_clone.clone();
            tokio::spawn(async move {
                let stream = MaybeTlsStream::Plain(stream);
                match accept_async(stream).await {
                    Ok(ws) => handle_connection(ws, bcast).await,
                    Err(e) => eprintln!("Error during WebSocket handshake: {}", e),
                }
            });
        }
    }))
}

async fn handle_connection(
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    bcast: Arc<BroadcastGroup>,
) {
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