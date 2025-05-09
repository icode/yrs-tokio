use futures_util::Sink;
use futures_util::stream::{SplitSink, SplitStream};
use rocket_ws::Message;
use rocket_ws::stream::DuplexStream;
use yrs_tokio::signaling::Message as SignalingMessage;
use yrs_tokio::{
    YrsExchange, YrsSink, YrsStream, impl_yrs_signal_stream, to_signaling_message, yrs_common_sink,
};

#[derive(YrsStream)]
pub struct YrsStream(SplitStream<DuplexStream>);
#[derive(YrsExchange)]
pub struct YrsSignalStream(SplitStream<DuplexStream>);

impl_yrs_signal_stream!(YrsSignalStream, item => to_signaling_message!(item, custom Message::Frame(frame) => SignalingMessage::Binary(frame.into_data())));
#[derive(YrsSink)]
pub struct YrsSink(SplitSink<DuplexStream, Message>);
#[yrs_common_sink]
impl Sink<SignalingMessage> for YrsSink {}

#[cfg(test)]
mod test {
    use crate::{YrsSink, YrsStream};
    use futures_util::{SinkExt, ready};
    use rocket::{State, get, routes};
    use rocket_ws::stream::DuplexStream;
    use rocket_ws::{Channel, WebSocket};
    use std::net::SocketAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::task::JoinHandle;
    use yrs::updates::encoder::Encode;
    use yrs::{GetString, Text, Transact};
    use yrs_tokio::broadcast::BroadcastGroup;
    use yrs_tokio::yrs_common_test;

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

    #[yrs_common_test]
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
}
