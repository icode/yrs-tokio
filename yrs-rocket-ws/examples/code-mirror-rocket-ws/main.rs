use rocket::fs::FileServer;
use rocket::{get, routes, State};
use rocket_ws::stream::DuplexStream;
use rocket_ws::{Channel, WebSocket};
use std::sync::Arc;
use futures_util::StreamExt;
use tokio::sync::{Mutex, RwLock};
use yrs::sync::Awareness;
use yrs::{Doc, Text, Transact};
use yrs_rocket_ws::{YrsSink, YrsStream};
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

    rocket::build()
        .configure(
            rocket::config::Config::figment()
                .merge(("address", "0.0.0.0"))
                .merge(("port", 8000)),
        )
        .manage(bcast)
        .mount("/", routes![ws_handler])
        .mount("/", FileServer::from(static_files_dir))
        .launch()
        .await
        .unwrap();
}

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
    let (sink, stream) = stream.split();
    let sink = Arc::new(Mutex::new(YrsSink::from(sink)));
    let stream = YrsStream::from(stream);

    let sub = bcast.subscribe(sink, stream);
    match sub.completed().await {
        Ok(_) => println!("broadcasting for channel finished successfully"),
        Err(e) => eprintln!("broadcasting for channel finished abruptly: {}", e),
    }
}
