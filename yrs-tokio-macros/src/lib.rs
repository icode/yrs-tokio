use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    Data, DeriveInput, Expr, Fields, Generics, Ident, ImplItem, ImplItemFn, ImplItemType, ItemImpl,
    ItemStruct, LitBool, LitStr, Token, Type, parse_macro_input,
};

/// Yrs tokio common test unit generator
/// # Examples
/// ```rust
///use std::net::SocketAddr;
///use std::str::FromStr;
///use yrs_axum_ws::{YrsSink, YrsStream};
///use axum::extract::ws::WebSocket;
///use axum::extract::{State, WebSocketUpgrade};
///use axum::response::Response;
///use futures_util::{ready, SinkExt, StreamExt};
///use std::sync::Arc;
///use axum::Router;
///use axum::routing::any;
///use tokio::sync::Mutex;
///use tokio::task;
///use tokio::task::JoinHandle;
///use yrs::updates::encoder::Encode;
///use yrs::{GetString, Text, Transact};
///use yrs_tokio::broadcast::BroadcastGroup;
///use yrs_tokio::yrs_common_test;
///
///#[yrs_common_test]
///async fn start_server(
///    addr: &str,
///    bcast: Arc<BroadcastGroup>,
///) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
///    let addr = SocketAddr::from_str(addr)?;
///
///    let app = Router::new()
///        .route("/my-room", any(ws_handler))
///        .with_state(bcast);
///
///    Ok(tokio::spawn(async move {
///        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
///        axum::serve(listener, app).await.unwrap();
///    }))
///}
///
///async fn ws_handler(
///    ws: WebSocketUpgrade,
///    State(bcast): State<Arc<BroadcastGroup>>,
///) -> Response {
///    ws.on_upgrade(move |socket| peer(socket, bcast))
///}
///
///async fn peer(ws: WebSocket, bcast: Arc<BroadcastGroup>) {
///    let (sink, stream) = ws.split();
///    let sink = Arc::new(Mutex::new(YrsSink::from(sink)));
///    let stream = YrsStream::from(stream);
///
///    let sub = bcast.subscribe(sink, stream);
///    match sub.completed().await {
///        Ok(_) => println!("broadcasting for channel finished successfully"),
///        Err(e) => eprintln!("broadcasting for channel finished abruptly: {}", e),
///    }
///}
/// ```
#[cfg(feature = "test-utils")]
#[proc_macro_attribute]
pub fn yrs_common_test(_: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input_fn.sig.ident;
    let original_fn_def = input_fn.to_token_stream();

    quote! {
        // ===============================================================
        // 在生成的代码块中定义辅助宏和类型，供测试函数使用
        // ===============================================================

        // 辅助宏: 用于委托 poll_* 方法调用到内部 sink，并进行错误转换
        macro_rules! delegate_poll_method {
            ($self:expr, $cx:expr, $method:ident $(($($arg:expr),*))?) => {{
                use ::std::task::Poll;
                use ::std::task::ready;
                let sink = unsafe { ::std::pin::Pin::new_unchecked(&mut $self.0) };
                let result = ready!(sink.$method($($($arg),*)?));
                match result {
                    Ok(_) => Poll::Ready(Ok(())),
                    Err(e) => Poll::Ready(Err(::yrs::sync::Error::Other(Box::new(e)))),
                }
            }};
        }

        // 辅助宏: 用于委托 Result 方法调用到内部 sink，并进行错误转换 (如 start_send)
        macro_rules! delegate_result_method {
            ($self:expr, $method:ident $(($($arg:expr),*))?) => {{
                let sink = unsafe { ::std::pin::Pin::new_unchecked(&mut $self.0) };
                let result = sink.$method($($($arg),*)?);
                match result {
                    Ok(_) => Ok(()),
                    Err(e) => Err(::yrs::sync::Error::Other(Box::new(e))),
                }
            }};
        }

        // 辅助宏: 用于委托 Stream 的 poll_next 方法调用到内部 stream，并进行错误转换和数据处理
        macro_rules! delegate_stream_poll_next {
            ($self:expr, $cx:expr) => {{
                use ::std::task::Poll;
                use ::std::task::ready;
                let stream = unsafe { ::std::pin::Pin::new_unchecked(&mut $self.0) };
                let result = ready!(stream.poll_next($cx));
                match result {
                    None => Poll::Ready(None),
                    Some(Ok(msg)) => Poll::Ready(Some(Ok(msg.into_data().into()))),
                    Some(Err(e)) => Poll::Ready(Some(Err(::yrs::sync::Error::Other(Box::new(e))))),
                }
            }};
        }


        // 定义 TungsteniteSink，并使用辅助宏简化其 Sink 实现
        struct TungsteniteSink(::futures_util::stream::SplitSink<::tokio_tungstenite::WebSocketStream<::tokio_tungstenite::MaybeTlsStream<::tokio::net::TcpStream>>, ::tokio_tungstenite::tungstenite::Message>);

        impl ::futures_util::Sink<Vec<u8>> for TungsteniteSink {
            type Error = ::yrs::sync::Error;

            fn poll_ready(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
            ) -> ::std::task::Poll<Result<(), Self::Error>> {
                delegate_poll_method!(self, cx, poll_ready(cx))
            }

            fn start_send(mut self: ::std::pin::Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
                delegate_result_method!(self, start_send(::tokio_tungstenite::tungstenite::Message::binary(item)))
            }

            fn poll_flush(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
            ) -> ::std::task::Poll<Result<(), Self::Error>> {
                delegate_poll_method!(self, cx, poll_flush(cx))
            }

            fn poll_close(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
            ) -> ::std::task::Poll<Result<(), Self::Error>> {
                delegate_poll_method!(self, cx, poll_close(cx))
            }
        }

        // 定义 TungsteniteStream，并使用辅助宏简化其 Stream 实现
        struct TungsteniteStream(::futures_util::stream::SplitStream<::tokio_tungstenite::WebSocketStream<::tokio_tungstenite::MaybeTlsStream<::tokio::net::TcpStream>>>);
        impl ::futures_util::Stream for TungsteniteStream {
            type Item = Result<Vec<u8>, ::yrs::sync::Error>;

            fn poll_next(mut self: ::std::pin::Pin<&mut Self>, cx: &mut ::std::task::Context<'_>) -> ::std::task::Poll<Option<Self::Item>> {
                delegate_stream_poll_next!(self, cx)
            }
        }

        // 定义 client 辅助函数
        async fn client(
            addr: &str,
            doc: ::yrs::Doc,
        ) -> Result<::yrs_tokio::connection::Connection<TungsteniteSink, TungsteniteStream>, Box<dyn std::error::Error>> {
            let (stream, _) = ::tokio_tungstenite::connect_async(addr).await?;
            let (sink, stream) = ::futures_util::stream::StreamExt::split(stream);
            let sink = TungsteniteSink(sink);
            let stream = TungsteniteStream(stream);
            Ok(::yrs_tokio::connection::Connection::new(
                ::std::sync::Arc::new(::tokio::sync::RwLock::new(::yrs::sync::Awareness::new(doc))),
                sink,
                stream,
            ))
        }

        // 定义 create_notifier 辅助函数
        fn create_notifier(doc: &::yrs::Doc) -> (::std::sync::Arc<::tokio::sync::Notify>, ::yrs::Subscription) {
            let n = ::std::sync::Arc::new(::tokio::sync::Notify::new());
            let sub = {
                let n = n.clone();
                doc.observe_update_v1(move |_, _| n.notify_waiters())
                    .unwrap()
            };
            (n, sub)
        }

        // 定义 TIMEOUT 常量
        const TIMEOUT: ::std::time::Duration = ::std::time::Duration::from_secs(5);

        // 定义辅助函数：设置客户端文档更新时自动发送消息 (修改为 async)
        async fn setup_client_update_propagation( // <--- 添加 async
            conn: &::yrs_tokio::connection::Connection<TungsteniteSink, TungsteniteStream>
        ) -> ::yrs::Subscription { // <--- 返回 Subscription
            let sink = conn.sink(); // Weak<Mutex<TungsteniteSink>>, 'static lifetime
            let awareness_arc = conn.awareness().clone(); // Arc<RwLock<Awareness<Doc>>>, 'static lifetime

            // 将获取 Doc 引用和设置 observe_update_v1 的同步操作移到阻塞任务中
            let sub_handle: ::tokio::task::JoinHandle<::yrs::Subscription> = ::tokio::task::spawn_blocking(move || { // <--- 使用 spawn_blocking
                 // 这个闭包在阻塞线程池上执行，可以安全地调用 blocking_read()
                 let awareness_guard = awareness_arc.blocking_read(); // <--- 现在安全了
                 let doc = awareness_guard.doc();
                 let inner_sink = sink.clone(); // 克隆 Weak 引用给 observer 闭包使用

                 // 设置 observe_update_v1 监听器
                 doc.observe_update_v1(move |_, e| {
                    // 这个回调函数可能在任意线程上被调用，取决于 yrs 内部的实现，
                    // 但内部生成新的异步任务是安全的方式。
                    let update = e.update.to_owned();
                    if let Some(sink) = inner_sink.upgrade() {
                         // 在异步运行时上生成新任务来发送消息
                         ::tokio::task::spawn(async move {
                            let msg = ::yrs::sync::Message::Sync(::yrs::sync::SyncMessage::Update(update))
                                .encode_v1();
                            let mut sink_guard = sink.lock().await;
                            sink_guard.send(msg).await.unwrap();
                        });
                    }
                 })
                 .unwrap() // 获取 Subscription
            });

            // 在 async 函数中等待阻塞任务的结果 (Subscription)
            sub_handle.await.unwrap() // <--- Await the JoinHandle
        }

        // 定义辅助函数：检查客户端文档中 "test" 文本的内容
        async fn check_client_text_content(
            conn: &::yrs_tokio::connection::Connection<TungsteniteSink, TungsteniteStream>,
            expected_str: &str,
        ) {
            let awareness = conn.awareness().read().await;
            let doc = awareness.doc();
            let text = doc.get_or_insert_text("test");
            let str = text.get_string(&doc.transact());
            ::core::assert_eq!(str, expected_str.to_string());
        }

         // 定义辅助函数：修改客户端或服务端文档中 "test" 文本的内容
         async fn apply_text_change(
             awareness: &::std::sync::Arc<::tokio::sync::RwLock<::yrs::sync::Awareness>>,
             change_str: &str,
         ) {
             let mut lock = awareness.write().await;
             let doc = lock.doc();
             let text = doc.get_or_insert_text("test");
             text.push(&mut doc.transact_mut(), change_str);
         }


        // ===============================================================
        // 原始被修饰的函数定义 (例如，这是服务端的启动函数)
        // ===============================================================
        #original_fn_def

        // ===============================================================
        // 使用上面定义的辅助项的测试用例
        // ===============================================================

        #[::tokio::test]
        async fn change_introduced_by_server_reaches_subscribed_clients() {
            let doc = ::yrs::Doc::with_client_id(1);
            let text = doc.get_or_insert_text("test");
            let awareness = ::std::sync::Arc::new(::tokio::sync::RwLock::new(::yrs::sync::Awareness::new(doc)));
            let bcast = ::yrs_tokio::broadcast::BroadcastGroup::new(awareness.clone(), 10).await;
            let _server = #fn_name("0.0.0.0:6600", ::std::sync::Arc::new(bcast)).await.unwrap();

            let doc = ::yrs::Doc::new();
            let (n, _sub) = create_notifier(&doc);
            let c1 = client("ws://localhost:6600/my-room", doc).await.unwrap();

            apply_text_change(&awareness, "abc").await;

            ::tokio::time::timeout(TIMEOUT, n.notified()).await.unwrap();

            check_client_text_content(&c1, "abc").await;
        }

        #[::tokio::test]
        async fn subscribed_client_fetches_initial_state() {
            let doc = ::yrs::Doc::with_client_id(1);
            let text = doc.get_or_insert_text("test");

            text.push(&mut doc.transact_mut(), "abc");

            let awareness = ::std::sync::Arc::new(::tokio::sync::RwLock::new(::yrs::sync::Awareness::new(doc)));
            let bcast = ::yrs_tokio::broadcast::BroadcastGroup::new(awareness.clone(), 10).await;
            let _server = #fn_name("0.0.0.0:6601", ::std::sync::Arc::new(bcast)).await.unwrap();

            let doc = ::yrs::Doc::new();
            let (n, _sub) = create_notifier(&doc);
            let c1 = client("ws://localhost:6601/my-room", doc).await.unwrap();

            ::tokio::time::timeout(TIMEOUT, n.notified()).await.unwrap();

            check_client_text_content(&c1, "abc").await;
        }

        #[::tokio::test]
        async fn changes_from_one_client_reach_others() {
            let doc = ::yrs::Doc::with_client_id(1);
            let _ = doc.get_or_insert_text("test");

            let awareness = ::std::sync::Arc::new(::tokio::sync::RwLock::new(::yrs::sync::Awareness::new(doc)));
            let bcast = ::yrs_tokio::broadcast::BroadcastGroup::new(awareness.clone(), 10).await;
            let _server = #fn_name("0.0.0.0:6602", ::std::sync::Arc::new(bcast)).await.unwrap();

            let d1 = ::yrs::Doc::with_client_id(2);
            let c1 = client("ws://localhost:6602/my-room", d1).await.unwrap();
            // 使用辅助函数设置客户端更新传播 (现在是 async，需要 .await)
            let _sub11 = setup_client_update_propagation(&c1).await;


            let d2 = ::yrs::Doc::with_client_id(3);
            let (n2, _sub2) = create_notifier(&d2);
            let c2 = client("ws://localhost:6602/my-room", d2).await.unwrap();

            apply_text_change(&c1.awareness(), "def").await;

            ::tokio::time::timeout(TIMEOUT, n2.notified()).await.unwrap();

            check_client_text_content(&c2, "def").await;
        }

        #[::tokio::test]
        async fn client_failure_doesnt_affect_others() {
            let doc = ::yrs::Doc::with_client_id(1);
            let _text = doc.get_or_insert_text("test");

            let awareness = ::std::sync::Arc::new(::tokio::sync::RwLock::new(::yrs::sync::Awareness::new(doc)));
            let bcast = ::yrs_tokio::broadcast::BroadcastGroup::new(awareness.clone(), 10).await;
            let _server = #fn_name("0.0.0.0:6603", ::std::sync::Arc::new(bcast)).await.unwrap();

            let d1 = ::yrs::Doc::with_client_id(2);
            let c1 = client("ws://localhost:6603/my-room", d1).await.unwrap();
            // 使用辅助函数设置客户端更新传播 (现在是 async，需要 .await)
            let _sub11 = setup_client_update_propagation(&c1).await;

            let d2 = ::yrs::Doc::with_client_id(3);
            let (n2, sub2) = create_notifier(&d2);
            let c2 = client("ws://localhost:6603/my-room", d2).await.unwrap();

            let d3 = ::yrs::Doc::with_client_id(4);
            let (n3, sub3) = create_notifier(&d3);
            let c3 = client("ws://localhost:6603/my-room", d3).await.unwrap();

            apply_text_change(&c1.awareness(), "abc").await;

            ::tokio::time::sleep(TIMEOUT).await;

            check_client_text_content(&c2, "abc").await;
            check_client_text_content(&c3, "abc").await;

            drop(c3);
            drop(n3);
            drop(sub3);
            drop(n2);
            drop(sub2);

            // 重新创建 notifier
             let (n2, _sub2) = {
                let a = c2.awareness().read().await;
                let doc = a.doc();
                create_notifier(doc)
            };

            apply_text_change(&c1.awareness(), "def").await;

            ::tokio::time::timeout(TIMEOUT, n2.notified()).await.unwrap();

            check_client_text_content(&c2, "abcdef").await;
        }
    }.into()
}
/// Yrs tokio Into/From generator
#[proc_macro_derive(YrsExchange)]
pub fn derive_yrs_exchange(input: TokenStream) -> TokenStream {
    // 传递 generics 到闭包
    derive_impl(input, "YrsExchange", |name, field_type, generics| {
        // 调用修改后的 quote_from_into
        TokenStream::from(quote_from_into(name, field_type, generics))
    })
}

/// Yrs tokio stream generator, use `into` argument defined convert method, use for not has message
/// # Examples
/// ```rust
/// use yrs_tokio_macros::yrs_stream;
/// use tokio_tungstenite::WebSocketStream;
/// use tokio::io::{AsyncRead, AsyncWrite};
/// use std::marker::Unpin;
/// use futures_util::stream::SplitStream;
///
/// #[yrs_stream(into=into_data().into(), exchange=false)]
/// pub struct YrsStream<S>(SplitStream<WebSocketStream<S>>)
/// where
///     S: AsyncRead + AsyncWrite + Unpin;
/// ```
#[proc_macro_attribute]
pub fn yrs_stream(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as MethodCallAttributeArgs);
    let call_target = args.into_target;
    let gen_exchange = args.exchange;

    let item_for_parsing = item.clone();
    let item_for_codegen = item;

    let input_struct = parse_macro_input!(item_for_parsing as ItemStruct);

    // 获取泛型信息
    let generics = input_struct.generics.clone();

    let original_struct_def = input_struct.to_token_stream();

    yrs_stream_code_gen(
        call_target,
        item_for_codegen,
        gen_exchange,
        Some(original_struct_def),
        generics,
    )
}

/// Yrs tokio stream generator, use convert message
/// # Examples
/// ```rust
/// use yrs_tokio_macros::YrsStream;
/// use std::marker::Unpin;
/// use axum::extract::ws::WebSocket;
/// use futures_util::stream::SplitStream;
///
///#[derive(YrsStream)]
///pub struct YrsStream(SplitStream<WebSocket>);
/// ```
#[proc_macro_derive(YrsStream)]
pub fn derive_yrs_stream(input: TokenStream) -> TokenStream {
    let default_target = CallTarget::SingleMethod(format_ident!("into"));
    let gen_exchange = true;

    let input_struct = parse_macro_input!(input as DeriveInput);
    let generics = input_struct.generics.clone();
    let input_for_derive_impl = input_struct.into_token_stream().into(); // 转换回 TokenStream 传递给 derive_impl

    yrs_stream_code_gen(
        default_target,
        input_for_derive_impl,
        gen_exchange,
        None,
        generics,
    )
}

/// Yrs tokio stream generator, use convert message, without exchange
/// # Examples
/// ```rust
/// use yrs_tokio_macros::YrsStreamOnly;
/// use std::marker::Unpin;
/// use axum::extract::ws::WebSocket;
/// use futures_util::stream::SplitStream;
///
///#[derive(YrsStreamOnly)]
///pub struct YrsStream(SplitStream<WebSocket>);
/// ```
#[proc_macro_derive(YrsStreamOnly)]
pub fn derive_yrs_stream_only(input: TokenStream) -> TokenStream {
    let default_target = CallTarget::SingleMethod(format_ident!("into"));
    let gen_exchange = false;

    let input_struct = parse_macro_input!(input as DeriveInput);
    let generics = input_struct.generics.clone();
    let input_for_derive_impl = input_struct.into_token_stream().into();

    yrs_stream_code_gen(
        default_target,
        input_for_derive_impl,
        gen_exchange,
        None,
        generics,
    )
}

#[derive(Clone)]
enum CallTarget {
    SingleMethod(Ident), // 存储单一方法的标识符 (如 `into_data`)
    MethodChain(Expr),   // 存储方法链的表达式 (如 `into_data().into()`)
}
struct MethodCallAttributeArgs {
    into_target: CallTarget,
    exchange: bool,
}

impl Parse for MethodCallAttributeArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut into_target: Option<CallTarget> = None;
        let mut exchange: bool = true;

        while !input.is_empty() {
            let lookahead = input.lookahead1();

            if lookahead.peek(Ident) && input.peek2(Token![=]) {
                let key: Ident = input.parse()?;
                let _eq_token: Token![=] = input.parse()?;

                if key == "into" {
                    let target_expr: Expr = input.parse()?;
                    let parsed_target = match target_expr {
                        Expr::Path(expr_path) => {
                            if expr_path.qself.is_none() && expr_path.path.segments.len() == 1 {
                                let segment =
                                    expr_path.clone().path.segments.into_iter().next().unwrap();
                                if segment.arguments.is_empty() {
                                    CallTarget::SingleMethod(segment.ident)
                                } else {
                                    CallTarget::MethodChain(Expr::Path(expr_path))
                                }
                            } else {
                                CallTarget::MethodChain(Expr::Path(expr_path))
                            }
                        }
                        _ => CallTarget::MethodChain(target_expr),
                    };
                    into_target = Some(parsed_target);
                } else if key == "exchange" {
                    let lit_bool: LitBool = input.parse()?;
                    exchange = lit_bool.value();
                } else {
                    return Err(input.error(format!("未知属性参数: `{}`", key)));
                }
            } else {
                return Err(input.error("期望 `key = value` 形式的属性参数"));
            }

            if !input.is_empty() {
                let lookahead = input.lookahead1();
                if lookahead.peek(Token![,]) {
                    let _: Token![,] = input.parse()?;
                } else {
                    return Err(input.error("属性参数之间期望用 `,` 分隔"));
                }
            }
        }

        let into_target = into_target.ok_or_else(|| input.error("属性中期望关键字 `into`"))?;

        Ok(MethodCallAttributeArgs {
            into_target,
            exchange,
        })
    }
}

fn yrs_stream_code_gen(
    call_target: CallTarget,
    input: TokenStream,
    gen_exchange: bool,
    preppend: Option<proc_macro2::TokenStream>,
    generics: Generics,
) -> TokenStream {
    derive_impl(input, "YrsStream", move |name, field_type, _| {
        let call_target = call_target.clone();
        let generics = generics.clone();

        let item_call_code = match &call_target {
            CallTarget::SingleMethod(method_ident) => {
                quote! { item.#method_ident() }
            }
            CallTarget::MethodChain(chain_expr) => {
                quote! { item.#chain_expr }
            }
        };

        let from_into: Option<proc_macro2::TokenStream> = if gen_exchange {
            Some(quote_from_into(name, field_type, &generics)) // 传递 generics 引用
        } else {
            None
        };

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        quote! {
            #preppend

            #from_into

            impl #impl_generics ::futures_core::Stream for #name #ty_generics #where_clause {
                type Item = Result<Vec<u8>, ::yrs::sync::Error>;

                fn poll_next(
                    mut self: ::core::pin::Pin<&mut Self>,
                    cx: &mut ::core::task::Context<'_>
                ) -> ::core::task::Poll<Option<Self::Item>> {
                    match ::core::pin::Pin::new(&mut self.0).poll_next(cx) {
                        ::core::task::Poll::Pending => ::core::task::Poll::Pending,
                        ::core::task::Poll::Ready(None) => ::core::task::Poll::Ready(None),
                        ::core::task::Poll::Ready(Some(res)) => match res {
                            Ok(item) => ::core::task::Poll::Ready(Some(Ok(#item_call_code))),
                            Err(e) => ::core::task::Poll::Ready(Some(Err(::yrs::sync::Error::Other(e.into())))),
                        },
                    }
                }
            }
        }.into()
    })
}

/// Yrs tokio sink generator
/// # Examples
/// ```rust
/// use yrs_tokio_macros::YrsSink;
/// use tokio_tungstenite::WebSocketStream;
/// use tokio::io::{AsyncRead, AsyncWrite};
/// use std::marker::Unpin;
/// use futures_util::stream::SplitSink;
/// use tokio_tungstenite::tungstenite::Message;
///
/// #[derive(YrsSink)]
/// pub struct YrsSink<S>(SplitSink<WebSocketStream<S>, Message>)
/// where
///     S: AsyncRead + AsyncWrite + Unpin + Send;
/// ```
#[proc_macro_derive(YrsSink)]
pub fn derive_yrs_sink(input: TokenStream) -> TokenStream {
    derive_yrs_sink_gen(input, true)
}

/// Yrs tokio sink generator without exchange
/// # Examples
/// ```rust
/// use yrs_tokio_macros::YrsSinkOnly;
/// use tokio_tungstenite::WebSocketStream;
/// use tokio::io::{AsyncRead, AsyncWrite};
/// use std::marker::Unpin;
/// use futures_util::stream::SplitSink;
/// use tokio_tungstenite::tungstenite::Message;
///
/// #[derive(YrsSinkOnly)]
/// pub struct YrsSink<S>(SplitSink<WebSocketStream<S>, Message>)
/// where
///     S: AsyncRead + AsyncWrite + Unpin + Send;
/// ```
#[proc_macro_derive(YrsSinkOnly)]
pub fn derive_yrs_sink_only(input: TokenStream) -> TokenStream {
    derive_yrs_sink_gen(input, false)
}

fn generate_poll_method_item(method_name: &str) -> ImplItemFn {
    let ident = format_ident!("{}", method_name);
    let inner_field_access = quote::quote!(self.0);

    syn::parse_quote! {
        fn #ident(
            mut self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<(), Self::Error>> {
            match ::core::pin::Pin::new(&mut #inner_field_access).#ident(cx) {
                ::core::task::Poll::Pending => ::core::task::Poll::Pending,
                ::core::task::Poll::Ready(Err(e)) => ::core::task::Poll::Ready(Err(::yrs::sync::Error::Other(e.into()))),
                ::core::task::Poll::Ready(_) => ::core::task::Poll::Ready(Ok(())),
            }
        }
    }
}

fn generate_start_send_method_item() -> ImplItemFn {
    let inner_field_access = quote::quote!(self.0);

    syn::parse_quote! {
        fn start_send(
            mut self: ::core::pin::Pin<&mut Self>,
            item: Vec<u8>,
        ) -> Result<(), Self::Error> {
            ::core::pin::Pin::new(&mut #inner_field_access).start_send(item.into())
                .map_err(|e| ::yrs::sync::Error::Other(e.into()))
        }
    }
}

fn derive_yrs_sink_gen(input: TokenStream, gen_exchange: bool) -> TokenStream {
    derive_impl(input, "YrsSink", move |name, field_type, generics| {
        let from_into: Option<proc_macro2::TokenStream> = if gen_exchange {
            Some(quote_from_into(name, field_type, generics))
        } else {
            None
        };

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let poll_ready_fn = generate_poll_method_item("poll_ready");
        let poll_flush_fn = generate_poll_method_item("poll_flush");
        let poll_close_fn = generate_poll_method_item("poll_close");
        let start_send_fn = generate_start_send_method_item();

        quote! {
            #from_into

            impl #impl_generics ::futures_util::Sink<Vec<u8>> for #name #ty_generics #where_clause {
                type Error = ::yrs::sync::Error;

                #poll_ready_fn
                #start_send_fn
                #poll_flush_fn
                #poll_close_fn
            }
        }
        .into()
    })
}

struct CommonSinkArgs {
    inner_field: LitStr,
}
impl Parse for CommonSinkArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut inner_field: Option<LitStr> = None;

        while !input.is_empty() {
            let lookahead = input.lookahead1();

            if !lookahead.peek(Ident) || !input.peek2(Token![=]) {
                return Err(input.error("Expected attribute argument `key = value`"));
            }

            let key: Ident = input.parse()?;
            let _eq_token: Token![=] = input.parse()?;

            if key == "inner" {
                if inner_field.is_some() {
                    return Err(input.error("Duplicate `inner` argument"));
                }
                inner_field = Some(input.parse()?);
            } else {
                return Err(input.error(format!("Unknown attribute argument: `{}`", key)));
            }

            if !input.is_empty() {
                let lookahead = input.lookahead1();
                if lookahead.peek(Token![,]) {
                    let _: Token![,] = input.parse()?;
                } else {
                    return Err(input.error("Attribute arguments must be separated by commas"));
                }
            }
        }

        let inner_field =
            inner_field.unwrap_or_else(|| LitStr::new("self.0", proc_macro2::Span::call_site()));

        Ok(CommonSinkArgs { inner_field })
    }
}

// 辅助函数：从 impl Sink<Item> for ... 头部解析出 Item 类型
fn get_sink_item_type(input: &ItemImpl) -> Result<Type, TokenStream> {
    // <--- 修改返回类型为 Type
    let trait_option = input.trait_.as_ref();
    let trait_ref =
        match trait_option {
            Some(tr) => tr,
            None => return Err(syn::Error::new_spanned(
                input,
                "yrs_common_sink must be applied to a Sink impl (e.g., `impl Sink<Item> for Type`)",
            )
            .to_compile_error()
            .into()),
        };
    let trait_path = trait_ref.1.clone();

    let last_segment_option = trait_path.segments.last();
    let last_segment = match last_segment_option {
        Some(seg) => seg,
        None => {
            return Err(syn::Error::new_spanned(&trait_path, "Invalid trait path")
                .to_compile_error()
                .into());
        }
    };

    if last_segment.ident != "Sink" {
        return Err(syn::Error::new_spanned(
            &last_segment,
            "yrs_common_sink must be applied to a Sink impl",
        )
        .to_compile_error()
        .into());
    }

    let args = match &last_segment.arguments {
        syn::PathArguments::AngleBracketed(args) => args,
        _ => {
            return Err(syn::Error::new_spanned(
                &last_segment.arguments,
                "Sink trait must have angle bracketed arguments",
            )
            .to_compile_error()
            .into());
        }
    };

    if args.args.len() != 1 {
        return Err(syn::Error::new_spanned(
            &args.args,
            "Sink trait must have exactly one generic argument (Item)",
        )
        .to_compile_error()
        .into());
    }

    let generic_arg = args.args.first().unwrap();
    match generic_arg {
        syn::GenericArgument::Type(ty) => Ok(ty.clone()),
        arg => Err(
            syn::Error::new_spanned(arg, "Sink trait argument must be a type")
                .to_compile_error()
                .into(),
        ),
    }
}

/// Simplifies implementing `futures_util::Sink` trait by generating boilerplate
/// methods and managing member order.
///
/// This attribute macro should be applied to an `impl Sink<Item> for Type` block.
/// It finds and reorders members within the impl block to match the
/// `futures_util::Sink` trait definition order: `type Error`, `fn poll_ready`,
/// `fn start_send`, `fn poll_flush`, `fn poll_close`, followed by any other
/// user-defined members.
///
/// # Attributes
///
/// - `inner = "expr"`: Required string literal specifying the expression to
///   access the inner Sink instance (e.g., `"self.0"` or `"self.my_field"`).
///   The default value is `"self.0"`.
///
/// # Required Impl Members
///
/// The macro looks for these members in the `impl` block:
///
/// - `type Error`: The associated error type. If not manually defined in the
///   impl block, the macro will default to `type Error = ::yrs::sync::Error;`.
/// - `fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error>`:
///   The method for sending an item into the sink. If not manually defined in the
///   impl block, the macro will attempt to generate a default implementation.
///   The default implementation depends on the `Item` type of the Sink being implemented:
///   - If `Item` is `SignalingMessage` (checked by name token), a specific `match`
///     based conversion body is generated, using unqualified names (`Message`, `Bytes`),
///     relying on user-provided `use` statements.
///   - For any other `Item` type, a generic `item.into().map_err(...)` body is generated.
///     This requires the Item type to implement `Into<InnerSinkItem>`.
///
/// # Generated Members
///
/// The macro generates these members if they are not manually defined in the impl block
/// (for `type Error` and `fn start_send`) or always generates them (`poll_*` methods):
///
/// - `type Error`: Generated if not manually defined. Defaults to `::yrs::sync::Error`.
/// - `fn poll_ready(...)`: Always generated. Delegates to the inner sink.
/// - `fn poll_flush(...)`: Always generated. Delegates to the inner sink.
/// - `fn poll_close(...)`: Always generated. Delegates to the inner sink.
/// - `fn start_send(...)`: Generated if not manually defined. Implementation depends on Item type.
///
/// # Member Order
///
/// All members in the `impl` block will be automatically reordered to match the
/// `futures_util::Sink` trait definition order:
/// `type Error`, `fn poll_ready`, `fn start_send`, `fn poll_flush`, `fn poll_close`,
/// followed by any other user-defined members (helper functions, consts, etc.).
#[proc_macro_attribute]
pub fn yrs_common_sink(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut impl_block = parse_macro_input!(item as ItemImpl);

    let args = parse_macro_input!(attr as CommonSinkArgs);
    let inner_expr_str = args.inner_field.value();

    let inner_expr: proc_macro2::TokenStream = match inner_expr_str.parse() {
        Ok(expr) => expr,
        Err(_) => {
            return syn::Error::new_spanned(args.inner_field, "Cannot parse `inner` expression")
                .to_compile_error()
                .into();
        }
    };

    let outer_sink_item_type = match get_sink_item_type(&impl_block) {
        Ok(ty) => ty.clone(),
        Err(e) => return e,
    };

    let mut user_start_send: Option<ImplItemFn> = None;
    let mut user_error_type_item: Option<ImplItemType> = None;
    let mut other_user_items: Vec<ImplItem> = Vec::new();

    let original_items = std::mem::take(&mut impl_block.items);
    for item in original_items {
        match item {
            ImplItem::Fn(f) if f.sig.ident == "start_send" => {
                if user_start_send.is_some() {
                    return syn::Error::new_spanned(&f.sig.ident, "Duplicate `start_send` method")
                        .to_compile_error()
                        .into();
                }
                user_start_send = Some(f);
            }
            ImplItem::Type(t) if t.ident == "Error" => {
                if user_error_type_item.is_some() {
                    return syn::Error::new_spanned(&t.ident, "Duplicate `Error` type")
                        .to_compile_error()
                        .into();
                }
                user_error_type_item = Some(t.clone());
            }
            _ => other_user_items.push(item),
        }
    }

    let final_error_item: ImplItemType = user_error_type_item.unwrap_or_else(|| {
        syn::parse_quote! {
            type Error = ::yrs::sync::Error;
        }
    });

    let start_send_method_item: ImplItemFn = match user_start_send {
        Some(method) => method,
        None => {
            let target_simple_signaling: Type = syn::parse_quote!(SignalingMessage);
            let target_fully_qualified_signaling: Type =
                syn::parse_quote!(::yrs_tokio::signaling::Message);
            let target_qualified_signaling: Type = syn::parse_quote!(yrs_tokio::signaling::Message);

            // Convert syn::Type to string for comparison since Type doesn't implement PartialEq
            let outer_type_str = quote::quote!(#outer_sink_item_type).to_string();
            let simple_type_str = quote::quote!(#target_simple_signaling).to_string();
            let fully_qualified_type_str =
                quote::quote!(#target_fully_qualified_signaling).to_string();
            let qualified_type_str = quote::quote!(#target_qualified_signaling).to_string();

            let is_targeted_signaling_message_type = outer_type_str == simple_type_str
                || outer_type_str == fully_qualified_type_str
                || outer_type_str == qualified_type_str;

            let default_body = if is_targeted_signaling_message_type {
                quote! {
                    let msg = match item {
                        ::yrs_tokio::signaling::Message::Text(txt) => Message::text(txt),
                        ::yrs_tokio::signaling::Message::Binary(bytes) => Message::binary(bytes),
                        ::yrs_tokio::signaling::Message::Ping => Message::Ping(Vec::default().into()),
                        ::yrs_tokio::signaling::Message::Pong => Message::Pong(Vec::default().into()),
                        ::yrs_tokio::signaling::Message::Close => Message::Close(None.into()),
                    };
                    if let Err(e) = ::core::pin::Pin::new(&mut #inner_expr).start_send(msg) {
                         Err(::yrs::sync::Error::Other(e.into()))
                    } else {
                        Ok(())
                    }
                }
            } else {
                // 生成更通用的默认 body (使用全限定名，但 Item 参数用短名)
                quote! {
                     use ::core::pin::Pin;
                     use ::futures_util::Sink;
                     Pin::new(&mut #inner_expr).start_send(item.into())
                         .map_err(|e| ::yrs::sync::Error::Other(e.into()))
                }
            };

            syn::parse_quote! {
                fn start_send(mut self: ::core::pin::Pin<&mut Self>, item: #outer_sink_item_type) -> Result<(), Self::Error> {
                    #default_body
                }
            }
        }
    };

    let generated_poll_ready = generate_poll_fn("poll_ready", &inner_expr);

    let generated_poll_flush = generate_poll_fn("poll_flush", &inner_expr);

    let generated_poll_close = generate_poll_fn("poll_close", &inner_expr);

    let mut final_items: Vec<ImplItem> = Vec::new();

    final_items.push(ImplItem::Type(final_error_item)); // 1. type Error
    final_items.push(ImplItem::Fn(generated_poll_ready)); // 2. poll_ready
    final_items.push(ImplItem::Fn(start_send_method_item)); // 3. start_send
    final_items.push(ImplItem::Fn(generated_poll_flush)); // 4. poll_flush
    final_items.push(ImplItem::Fn(generated_poll_close)); // 5. poll_close

    final_items.extend(other_user_items);

    impl_block.items = final_items;

    quote!(#impl_block).into()
}

fn generate_poll_fn(method: &str, inner_expr: &proc_macro2::TokenStream) -> ImplItemFn {
    let ident = format_ident!("{}", method);

    syn::parse_quote! {
        fn #ident(
            mut self: ::core::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            match ::core::pin::Pin::new(&mut #inner_expr).#ident(cx) {
                std::task::Poll::Pending => std::task::Poll::Pending,
                std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(::yrs::sync::Error::Other(e.into()))),
                std::task::Poll::Ready(_) => std::task::Poll::Ready(Ok(())),
            }
        }
    }
}

fn quote_from_into(
    name: &Ident,
    field_type: &Type,
    generics: &Generics,
) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics ::core::convert::From<#field_type> for #name #ty_generics #where_clause {
            fn from(stream: #field_type) -> Self {
                #name(stream)
            }
        }

        impl #impl_generics ::core::convert::Into<#field_type> for #name #ty_generics #where_clause {
            fn into(self) -> #field_type {
                self.0
            }
        }
    }
}

/// 通用 derive 宏逻辑
fn derive_impl<F>(input: TokenStream, _macro_name: &str, f: F) -> TokenStream
where
    F: FnOnce(&Ident, &Type, &Generics) -> TokenStream, // 闭包签名修改
{
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics; // 获取泛型信息

    let field_type = match get_field_type(&input) {
        Ok(ty) => ty,
        Err(err) => return err,
    };

    // 调用闭包时传递泛型信息
    let expanded = f(name, field_type, generics);
    TokenStream::from(expanded)
}

/// 获取单字段 tuple struct 的字段类型
fn get_field_type(input: &DeriveInput) -> Result<&Type, TokenStream> {
    Ok(match &input.data {
        Data::Struct(data_struct) => {
            if let Fields::Unnamed(fields_unnamed) = &data_struct.fields {
                if fields_unnamed.unnamed.len() == 1 {
                    &fields_unnamed.unnamed.first().unwrap().ty
                } else {
                    return Err(syn::Error::new_spanned(
                        &input.ident,
                        "can only be derived for tuple structs with one field",
                    )
                    .to_compile_error()
                    .into());
                }
            } else {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "can only be derived for tuple structs with unnamed fields",
                )
                .to_compile_error()
                .into());
            }
        }
        _ => {
            return Err(
                syn::Error::new_spanned(&input.ident, "can only be derived for structs")
                    .to_compile_error()
                    .into(),
            );
        }
    })
}
