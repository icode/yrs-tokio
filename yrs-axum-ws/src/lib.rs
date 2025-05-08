use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::Sink;
use yrs_tokio::signaling::Message as SignalingMessage;
use yrs_tokio::{impl_yrs_signal_stream, to_signaling_message, yrs_common_sink, YrsExchange, YrsSink, YrsStream};

#[derive(YrsStream)]
pub struct YrsStream(SplitStream<WebSocket>);
#[derive(YrsExchange)]
pub struct YrsSignalStream(SplitStream<WebSocket>);

impl_yrs_signal_stream!(YrsSignalStream, item => to_signaling_message!(item));

#[derive(YrsSink)]
pub struct YrsSink(SplitSink<WebSocket, Message>);
#[yrs_common_sink]
impl Sink<SignalingMessage> for YrsSink {}

#[cfg(test)]
mod test {
    use crate::{YrsSink, YrsStream};
    use axum::extract::ws::WebSocket;
    use axum::extract::{State, WebSocketUpgrade};
    use axum::response::Response;
    use axum::routing::any;
    use axum::Router;
    use futures_util::{ready, SinkExt, StreamExt};
    use std::net::SocketAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::task;
    use tokio::task::JoinHandle;
    use yrs::updates::encoder::Encode;
    use yrs::{GetString, Text, Transact};
    use yrs_tokio::broadcast::BroadcastGroup;
    use yrs_tokio::yrs_common_test;

    #[yrs_common_test]
    async fn start_server(
        addr: &str,
        bcast: Arc<BroadcastGroup>,
    ) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
        let _ = tracing_subscriber::fmt::try_init();
        let addr = SocketAddr::from_str(addr)?;

        let app = Router::new()
            .route("/my-room", any(ws_handler))
            .with_state(bcast);

        Ok(tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        }))
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
}
