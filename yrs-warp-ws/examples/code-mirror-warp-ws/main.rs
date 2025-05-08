use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use warp::ws::{WebSocket, Ws};
use warp::{Filter, Rejection, Reply};
use yrs::sync::Awareness;
use yrs::{Doc, Text, Transact};
use yrs_warp_ws::{YrsSink, YrsStream};
use yrs_tokio::AwarenessRef;
use yrs_tokio::broadcast::BroadcastGroup;


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

    let static_files = warp::get().and(warp::fs::dir(static_files_dir));

    let ws = warp::path("my-room")
        .and(warp::ws())
        .and(warp::any().map(move || bcast.clone()))
        .and_then(ws_handler);

    let routes = ws.or(static_files);

    warp::serve(routes).run(([0, 0, 0, 0], 8000)).await;
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
