use futures_util::StreamExt;
use rocket::fs::FileServer;
use rocket::{get, routes, State};
use rocket_ws::stream::DuplexStream;
use rocket_ws::{Channel, WebSocket};
use yrs_rocket_ws::{YrsSignalStream, YrsSink};
use yrs_tokio::signaling::{signaling_connection, SignalingService};


#[tokio::main]
async fn main() {
    let static_files_dir: String = format!("{}/../examples/webrtc-signaling-server/frontend/dist", env!("CARGO_MANIFEST_DIR"));
    
    let signaling = SignalingService::new();

    rocket::build()
        .configure(
            rocket::config::Config::figment()
                .merge(("address", "0.0.0.0"))
                .merge(("port", 8000)),
        )
        .manage(signaling)
        .mount("/", routes![ws_handler])
        .mount("/", FileServer::from(static_files_dir))
        .launch()
        .await
        .unwrap();
}

#[get("/signaling")]
fn ws_handler(ws: WebSocket, svc: &State<SignalingService>) -> Channel<'_> {
    let svc = svc.inner().clone();

    ws.channel(move |stream| {
        Box::pin(async move {
            peer(stream, svc).await;

            Ok(())
        })
    })
}

async fn peer(stream: DuplexStream, svc: SignalingService) {
    println!("new incoming signaling connection");
    let (sink, stream) = stream.split();
    let sink = YrsSink::from(sink);
    let stream = YrsSignalStream::from(stream);
    match signaling_connection(sink, stream, svc).await {
        Ok(_) => println!("signaling connection stopped"),
        Err(e) => eprintln!("signaling connection failed: {}", e),
    }
}
