use futures_util::stream::{SplitSink, SplitStream};
use futures_util::Sink;
use std::pin::Pin;
use warp::ws::{Message, WebSocket};
use yrs::sync::Error;
use yrs_tokio::signaling::Message as SignalingMessage;
use yrs_tokio::{impl_yrs_signal_stream, yrs_common_sink, YrsExchange, YrsStream};

#[derive(YrsStream)]
pub struct YrsStream(SplitStream<WebSocket>);

#[derive(YrsExchange)]
pub struct YrsSignalStream(SplitStream<WebSocket>);

impl_yrs_signal_stream!(YrsSignalStream, item => {
    if item.is_text() {
        SignalingMessage::Text(item.to_str().unwrap().to_owned())
    } else if item.is_binary() {
        SignalingMessage::Binary(item.into_bytes())
    } else if item.is_ping() {
        SignalingMessage::Ping
    } else if item.is_pong() {
        SignalingMessage::Pong
    } else {
        SignalingMessage::Close
    }
});

#[derive(YrsExchange)]
pub struct YrsSink(SplitSink<WebSocket, Message>);

#[yrs_common_sink]
impl Sink<Vec<u8>> for YrsSink {
    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        if let Err(e) = Pin::new(&mut self.0).start_send(Message::binary(item)) {
            Err(Error::Other(e.into()))
        } else {
            Ok(())
        }
    }
}

#[yrs_common_sink]
impl Sink<SignalingMessage> for YrsSink {
    fn start_send(mut self: Pin<&mut Self>, item: SignalingMessage) -> Result<(), Self::Error> {
        let msg = match item {
            SignalingMessage::Text(txt) => Message::text(txt),
            SignalingMessage::Binary(bytes) => Message::binary(bytes),
            SignalingMessage::Ping => Message::ping(Vec::default()),
            SignalingMessage::Pong => Message::pong(Vec::default()),
            SignalingMessage::Close => Message::close(),
        };
        if let Err(e) = Pin::new(&mut self.0).start_send(msg) {
            Err(Error::Other(e.into()))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{YrsSink, YrsStream};
    use futures_util::{ready, SinkExt, StreamExt};
    use std::net::SocketAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::task;
    use tokio::task::JoinHandle;
    use warp::ws::{WebSocket, Ws};
    use warp::{Filter, Rejection, Reply};
    use yrs::updates::encoder::Encode;
    use yrs::{GetString, Text, Transact};
    use yrs_tokio::broadcast::BroadcastGroup;
    use yrs_tokio::yrs_common_test;

    #[yrs_common_test]
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
}
