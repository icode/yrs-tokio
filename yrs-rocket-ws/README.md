# yrs-rocket-ws

> Yrs message exchange protocol base on Rocket websocket

```rust
use yrs_rocket_ws::{YrsSink, YrsStream};

use rocket::{get, routes, State};
use rocket_ws::stream::DuplexStream;
use rocket_ws::{Channel, WebSocket};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use tokio::task::JoinHandle;
use yrs_tokio::broadcast::BroadcastGroup;

#[get("/my-room")]
fn ws_handler(ws: WebSocket, bcast: &State<Arc<BroadcastGroup>>) -> Channel<'_> {
    let bcast = bcast.inner();

    ws.channel(move |stream| {
        Box::pin(async move {
            peer(stream, bcast).await;

            Ok(())
        })
    })
}

async fn peer(stream: DuplexStream, bcast: &Arc<BroadcastGroup>) {
    use rocket::futures::StreamExt;
    let (sink, stream) = stream.split();
    let sink = Arc::new(Mutex::new(YrsSink::from(sink)));
    let stream = YrsStream::from(stream);

    let sub = bcast.subscribe(sink, stream);
    match sub.completed().await {
        Ok(_) => println!("broadcasting for channel finished successfully"),
        Err(e) => eprintln!("broadcasting for channel finished abruptly: {}", e),
    }
}

async fn start_server(
    addr: &str,
    bcast: Arc<BroadcastGroup>,
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    let addr = SocketAddr::from_str(addr)?;

    let rocket_handle = tokio::spawn(async move {
        let _rocket = rocket::build()
            .configure(
                rocket::config::Config::figment()
                    .merge(("address", addr.ip().to_string()))
                    .merge(("port", addr.port())),
            )
            .manage(bcast.clone()) // 将 BroadcastGroup 放入 Rocket 的状态管理
            .mount("/", routes![ws_handler])
            .launch()
            .await;
    });

    Ok(rocket_handle)
}
```