use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use yrs::sync::Error;

const PING_TIMEOUT: Duration = Duration::from_secs(30);

/// 消息类型
#[derive(Debug, Clone)]
pub enum Message {
    /// 文本消息
    Text(String),
    /// 二进制消息
    Binary(Vec<u8>),
    /// Ping消息
    Ping,
    /// Pong消息
    Pong,
    /// 关闭消息
    Close,
}

pub const PING_MSG: &str = r#"{"type":"ping"}"#;
pub const PONG_MSG: &str = r#"{"type":"pong"}"#;

impl Message {

    pub fn is_text(&self) -> bool {
        matches!(self, Message::Text(_))
    }

    pub fn is_binary(&self) -> bool {
        matches!(self, Message::Binary(_))
    }

    pub fn is_ping(&self) -> bool {
        matches!(self, Message::Ping)
    }

    pub fn is_pong(&self) -> bool {
        matches!(self, Message::Pong)
    }

    pub fn is_close(&self) -> bool {
        matches!(self, Message::Close)
    }

    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Message::Text(s) => s.into_bytes(),
            Message::Binary(b) => b,
            Message::Ping => Vec::new(),
            Message::Pong => Vec::new(),
            Message::Close => Vec::new(),
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        if bytes.is_empty() {
            return Message::Close;
        }
        if let Ok(s) = String::from_utf8(bytes.clone()) {
            Message::Text(s)
        } else {
            Message::Binary(bytes)
        }
    }
}

/// Signaling service is used by y-webrtc protocol in order to exchange WebRTC offerings between
/// clients subscribing to particular rooms.
#[derive(Clone)]
pub struct SignalingService(Topics);

impl SignalingService
{
    pub fn new() -> Self {
        SignalingService(Arc::new(RwLock::new(Default::default())))
    }

    pub async fn publish(&self, topic: &str, msg: Message) -> Result<(), Error> {
        let mut failed = Vec::new();
        {
            let topics = self.0.read().await;
            if let Some(subs) = topics.get(topic) {
                let client_count = subs.len();
                tracing::info!("publishing message to {client_count} clients: {msg:?}");
                for sub in subs {
                    if let Err(e) = sub.try_send(msg.clone()).await {
                        tracing::info!("failed to send {msg:?}: {e}");
                        failed.push(sub.clone());
                    }
                }
            }
        }
        if !failed.is_empty() {
            let mut topics = self.0.write().await;
            if let Some(subs) = topics.get_mut(topic) {
                for f in failed {
                    subs.remove(&f);
                }
            }
        }
        Ok(())
    }

    pub async fn close_topic(&self, topic: &str) -> Result<(), Error> {
        let mut topics = self.0.write().await;
        if let Some(subs) = topics.remove(topic) {
            for sub in subs {
                if let Err(e) = sub.close().await {
                    tracing::warn!("failed to close connection on topic '{topic}': {e}");
                }
            }
        }
        Ok(())
    }

    pub async fn close(self) -> Result<(), Error> {
        let mut topics = self.0.write_owned().await;
        let mut all_conns = HashSet::new();
        for (_, subs) in topics.drain() {
            for sub in subs {
                all_conns.insert(sub);
            }
        }

        for conn in all_conns {
            if let Err(e) = conn.close().await {
                tracing::warn!("failed to close connection: {e}");
            }
        }

        Ok(())
    }
}

impl Default for SignalingService
{
    fn default() -> Self {
        Self::new()
    }
}

type Topics = Arc<RwLock<HashMap<Arc<str>, HashSet<SignalSink>>>>;

type DynSink = dyn Sink<Message, Error = Error> + Send + Sync + Unpin;

#[derive(Clone)]
struct SignalSink(Arc<Mutex<Pin<Box<DynSink>>>>);

impl SignalSink {
    pub fn new<S>(sink: S) -> Self
    where
        S: Sink<Message, Error = Error> + Send + Sync + Unpin + 'static,
    {
        SignalSink(Arc::new(Mutex::new(Box::pin(sink))))
    }

    pub async fn try_send(&self, msg: Message) -> Result<(), Error> {
        let mut sink = self.0.lock().await;
        if let Err(e) = sink.as_mut().send(msg).await {
            sink.close().await?;
            Err(e)
        } else {
            Ok(())
        }
    }

    pub async fn close(&self) -> Result<(), Error> {
        let mut sink = self.0.lock().await;
        sink.as_mut().close().await
    }
}

impl Hash for SignalSink
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        let ptr = Arc::as_ptr(&self.0) as usize;
        ptr.hash(state);
    }
}

impl PartialEq<Self> for SignalSink
{
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for SignalSink {}

/// Handle incoming signaling connection - it's a connection used by y-webrtc protocol
/// to exchange offering metadata between y-webrtc peers. It also manages topic/room access.
pub async fn signaling_connection<S, T>(
    sink: S,
    mut stream: T,
    service: SignalingService,
) -> Result<(), Error>
where
    S: Sink<Message, Error = Error> + Send + Sync + Unpin + 'static,
    T: Stream<Item = Result<Message, Error>> + Unpin + Send + 'static,
{
    let topics_ref = &service.0;
    let signal_sink = SignalSink::new(sink);
    let mut ping_interval = interval(PING_TIMEOUT);
    let mut state = ConnState::default();
    loop {
        select! {
            _ = ping_interval.tick() => {
                if !state.pong_received {
                    signal_sink.close().await?;
                    drop(ping_interval);
                    return Ok(());
                } else {
                    state.pong_received = false;
                    if let Err(e) = signal_sink.try_send(Message::Ping).await {
                        signal_sink.close().await?;
                        return Err(e);
                    }
                }
            },
            res = stream.next() => {
                match res {
                    None => {
                        signal_sink.close().await?;
                        return Ok(());
                    },
                    Some(Err(e)) => {
                        signal_sink.close().await?;
                        return Err(e);
                    },
                    Some(Ok(msg)) => {
                        process_msg::<S>(msg, &signal_sink, &mut state, &topics_ref).await?;
                    }
                }
            }
        }
    }
}

async fn process_msg<S>(
    msg: Message,
    sink: &SignalSink,
    state: &mut ConnState,
    topics: &Topics,
) -> Result<(), Error>
where
    S: Sink<Message, Error = Error> + Send + Sync + Unpin + 'static,
{
    match msg {
        Message::Text(json) => {
            if let Ok(signal) = serde_json::from_str::<Signal>(&json) {
                match signal {
                    Signal::Subscribe { topics: topic_names } => {
                        if !topic_names.is_empty() {
                            let mut topics = topics.write().await;
                            for topic in topic_names {
                                tracing::trace!("subscribing new client to '{topic}'");
                                if let Some((key, _)) = topics.get_key_value(topic) {
                                    state.subscribed_topics.insert(key.clone());
                                    let subs = topics.get_mut(topic).unwrap();
                                    subs.insert(sink.clone());
                                } else {
                                    let topic: Arc<str> = topic.into();
                                    state.subscribed_topics.insert(topic.clone());
                                    let mut subs = HashSet::new();
                                    subs.insert(sink.clone());
                                    topics.insert(topic, subs);
                                };
                            }
                        }
                    }
                    Signal::Unsubscribe { topics: topic_names } => {
                        if !topic_names.is_empty() {
                            let mut topics = topics.write().await;
                            for topic in topic_names {
                                if let Some(subs) = topics.get_mut(topic) {
                                    tracing::trace!("unsubscribing client from '{topic}'");
                                    subs.remove(&sink);
                                }
                            }
                        }
                    }
                    Signal::Publish { topic } => {
                        let mut failed = Vec::new();
                        {
                            let topics = topics.read().await;
                            if let Some(receivers) = topics.get(topic) {
                                let client_count = receivers.len();
                                tracing::trace!(
                                    "publishing on {client_count} clients at '{topic}': {json}"
                                );
                                for receiver in receivers.iter() {
                                    if let Err(e) = receiver.try_send(Message::Text(json.clone())).await {
                                        tracing::info!(
                                            "failed to publish message {json} on '{topic}': {e}"
                                        );
                                        failed.push(receiver.clone());
                                    }
                                }
                            }
                        }
                        if !failed.is_empty() {
                            let mut topics = topics.write().await;
                            if let Some(receivers) = topics.get_mut(topic) {
                                for f in failed {
                                    receivers.remove(&f);
                                }
                            }
                        }
                    }
                    Signal::Ping => {
                        tracing::trace!("received text ping, sending pong");
                        sink.try_send(Message::Text(PONG_MSG.into())).await?;
                    }
                    Signal::Pong => {
                        tracing::trace!("received text pong, sending ping");
                        sink.try_send(Message::Text(PING_MSG.into())).await?;
                    }
                }
            }
        }
        Message::Binary(data) => {
            tracing::trace!("received binary message: {} bytes", data.len());
            sink.try_send(Message::Binary(data)).await?;
        }
        Message::Ping => {
            tracing::trace!("received ping, sending pong");
            sink.try_send(Message::Pong).await?;
        }
        Message::Pong => {
            tracing::trace!("received pong, ignore");
        }
        Message::Close => {
            tracing::trace!("received close message, cleaning up subscriptions");
            let mut topics = topics.write().await;
            for topic in state.subscribed_topics.drain() {
                if let Some(subs) = topics.get_mut(&topic) {
                    subs.remove(&sink);
                    if subs.is_empty() {
                        topics.remove(&topic);
                    }
                }
            }
            state.closed = true;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct ConnState {
    closed: bool,
    pong_received: bool,
    subscribed_topics: HashSet<Arc<str>>,
}

impl Default for ConnState {
    fn default() -> Self {
        ConnState {
            closed: false,
            pong_received: true,
            subscribed_topics: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum Signal<'a> {
    #[serde(rename = "publish")]
    Publish { topic: &'a str },
    #[serde(rename = "subscribe")]
    Subscribe { topics: Vec<&'a str> },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { topics: Vec<&'a str> },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "pong")]
    Pong,
}

#[macro_export]
macro_rules! impl_signal_stream_body {
    ($item_pat:pat => $item_expr:expr) => {
        type Item = Result<$crate::signaling::Message, yrs::sync::Error>;

        fn poll_next(
            mut self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Option<Self::Item>> {
            let inner = ::core::pin::Pin::new(&mut self.0);
            match inner.poll_next(cx) {
                ::core::task::Poll::Ready(Some(Ok($item_pat))) => {
                    ::core::task::Poll::Ready(Some(Ok($item_expr)))
                },
                ::core::task::Poll::Ready(Some(Err(e))) => {
                    ::core::task::Poll::Ready(Some(Err(yrs::sync::Error::Other(e.into()))))
                },
                ::core::task::Poll::Ready(None) => ::core::task::Poll::Ready(None),
                ::core::task::Poll::Pending => ::core::task::Poll::Pending,
            }
        }
    };
}

#[macro_export]
macro_rules! impl_yrs_signal_stream {
    // 带 external where 子句的泛型
    ($name:ident<$($gen:ident),+> where $($where_clause:tt)+, $item_pat:pat => $item_expr:expr) => {
        impl<$($gen),+> ::futures::stream::Stream for $name<$($gen),+>
        where
            $($gen: ::tokio::io::AsyncRead + ::tokio::io::AsyncWrite + ::core::marker::Unpin),+,
            $($($where_clause)+)?
        {
            $crate::impl_signal_stream_body!($item_pat => $item_expr);
        }
    };

    // 不带 external where 的泛型
    ($name:ident<$($gen:ident),+>, $item_pat:pat => $item_expr:expr) => {
        impl<$($gen),+> ::futures::stream::Stream for $name<$($gen),+>
        where
            $($gen: ::tokio::io::AsyncRead + ::tokio::io::AsyncWrite + ::core::marker::Unpin),+
        {
            $crate::impl_signal_stream_body!($item_pat => $item_expr);
        }
    };

    // 无泛型参数
    ($name:ident, $item_pat:pat => $item_expr:expr) => {
        impl ::futures::stream::Stream for $name {
            $crate::impl_signal_stream_body!($item_pat => $item_expr);
        }
    };
}

#[macro_export]
macro_rules! to_signaling_message_common {
    ($item:ident, custom $($user_logic:tt)*) => {
        match $item {
            Message::Text(text) => SignalingMessage::Text(text.to_string()),
            Message::Binary(bytes) => SignalingMessage::Binary(bytes.into()),
            Message::Ping(_) => SignalingMessage::Ping,
            Message::Pong(_) => SignalingMessage::Pong,
            Message::Close(_) => SignalingMessage::Close,
            $($user_logic)*
        }
    };
}

#[macro_export]
macro_rules! to_signaling_message {
    ($item:ident, frame) => {
        $crate::to_signaling_message_common!($item, custom Message::Frame(frame) => SignalingMessage::Binary(frame.into_payload().into()),)
    };

    ($item:ident, custom $($user_logic:tt)+) => {
         $crate::to_signaling_message_common!($item, custom $($user_logic)+)
    };

    ($item:ident) => {
        $crate::to_signaling_message_common!($item, custom)
    };
}