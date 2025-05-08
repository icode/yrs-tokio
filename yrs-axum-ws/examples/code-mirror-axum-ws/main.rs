use axum::extract::ws::WebSocket;
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tower_http::services::ServeDir;
use yrs::sync::Awareness;
use yrs::{Doc, Text, Transact};
use yrs_axum_ws::{YrsSink, YrsStream};
use yrs_tokio::broadcast::BroadcastGroup;
use yrs_tokio::AwarenessRef;


#[tokio::main]
async fn main() {
    let static_files_dir: String = format!("{}/../examples/code-mirror/frontend/dist", env!("CARGO_MANIFEST_DIR"));
    
    // We're using a single static document shared among all the peers.
    let awareness: AwarenessRef = {
        let doc = Doc::new();
        {
            // pre-initialize code mirror document with some text
            let txt = doc.get_or_insert_text("codemirror");
            let mut txn = doc.transact_mut();
            txt.push(
                &mut txn,
                r#"function hello() {
  console.log('hello world');
}"#,
            );
        }
        Arc::new(RwLock::new(Awareness::new(doc)))
    };

    // open a broadcast group that listens to awareness and document updates
    // and has a pending message buffer of up to 32 updates
    let bcast = Arc::new(BroadcastGroup::new(awareness.clone(), 32).await);

    let app = Router::new()
        .route("/my-room", any(ws_handler))
        .with_state(bcast)
        .fallback_service(ServeDir::new(static_files_dir));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
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
