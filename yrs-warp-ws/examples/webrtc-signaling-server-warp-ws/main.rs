use futures_util::StreamExt;
use tracing::Level;
use warp::ws::{WebSocket, Ws};
use warp::{Filter, Rejection, Reply};
use yrs_tokio::signaling::{signaling_connection, SignalingService};
use yrs_warp_ws::{YrsSignalStream, YrsSink};

#[tokio::main]
async fn main() {
    let static_files_dir: String = format!("{}/../examples/webrtc-signaling-server/frontend/dist", env!("CARGO_MANIFEST_DIR"));
    
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .init();

    let signaling = SignalingService::new();

    let static_files = warp::get().and(warp::fs::dir(static_files_dir));

    let ws = warp::path("signaling")
        .and(warp::ws())
        .and(warp::any().map(move || signaling.clone()))
        .and_then(ws_handler);

    let routes = ws.or(static_files);

    warp::serve(routes).run(([0, 0, 0, 0], 8000)).await;
}

async fn ws_handler(ws: Ws, svc: SignalingService) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| peer(socket, svc)))
}

async fn peer(ws: WebSocket, svc: SignalingService) {
    println!("new incoming signaling connection");
    let (sink, stream) = ws.split();
    let sink = YrsSink::from(sink);
    let stream = YrsSignalStream::from(stream);
    match signaling_connection(sink, stream, svc).await {
        Ok(_) => println!("signaling connection stopped"),
        Err(e) => eprintln!("signaling connection failed: {}", e),
    }
}
