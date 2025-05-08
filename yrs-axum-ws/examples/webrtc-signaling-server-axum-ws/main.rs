use axum::extract::ws::WebSocket;
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use futures_util::StreamExt;
use tower_http::services::ServeDir;
use yrs_axum_ws::{YrsSignalStream, YrsSink};
use yrs_tokio::signaling::{signaling_connection, SignalingService};


#[tokio::main]
async fn main() {
    let static_files_dir: String = format!("{}/../examples/webrtc-signaling-server/frontend/dist", env!("CARGO_MANIFEST_DIR"));
    
    let signaling = SignalingService::new();

    let app = Router::new()
        .route("/signaling", any(ws_handler))
        .with_state(signaling)
        .fallback_service(ServeDir::new(static_files_dir));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, State(svc): State<SignalingService>) -> Response {
    ws.on_upgrade(move |socket| peer(socket, svc))
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
