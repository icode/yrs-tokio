use futures_util::stream::{SplitSink, SplitStream};
use futures_util::Sink;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use yrs_tokio::signaling::Message as SignalingMessage;
use yrs_tokio::{impl_yrs_signal_stream, to_signaling_message, yrs_common_sink, yrs_stream, YrsExchange, YrsSink};

#[yrs_stream(into=into_data().into())]
pub struct YrsStream<S>(SplitStream<WebSocketStream<S>>)
where
    S: AsyncRead + AsyncWrite + Unpin;

#[derive(YrsExchange)]
pub struct YrsSignalStream<S>(SplitStream<WebSocketStream<S>>)
where
    S: AsyncRead + AsyncWrite + Unpin;

impl_yrs_signal_stream!(YrsSignalStream<S>, item => to_signaling_message!(item, frame));

#[derive(YrsSink)]
pub struct YrsSink<S>(SplitSink<WebSocketStream<S>, Message>)
where
    S: AsyncRead + AsyncWrite + Unpin;
#[yrs_common_sink]
impl<S> Sink<SignalingMessage> for YrsSink<S> where S: AsyncRead + AsyncWrite + Unpin {}

#[cfg(test)]
mod test {
    use crate::{YrsSink, YrsStream};
    use futures_util::{ready, SinkExt, StreamExt};
    use std::net::SocketAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::Mutex;
    use tokio::task;
    use tokio::task::JoinHandle;
    use tokio_tungstenite::{accept_async, MaybeTlsStream, WebSocketStream};
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
}
