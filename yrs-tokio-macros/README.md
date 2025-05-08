# yrs-tokio-macros

> macro utils for yrs-tokio

## #[derive(YrsExchange)]

Yrs tokio Into/From generator.

```rust
use yrs_tokio_macros::YrsExchange;

#[derive(YrsExchange)]
pub struct YrsSink(SplitSink<WebSocket, AxumMessage>);
```

## #[derive(YrsStream)]

Yrs tokio stream generator, use `Into<Vec<u8>>` convert message.

> This macro requires users to add a `futures-core` 0.3 dependency.

```rust
use yrs_tokio_macros::YrsStream;

#[derive(YrsStream)]
pub struct YrsStream(SplitStream<WebSocket>);
```

## #[yrs_stream(into={into_method})]

Yrs tokio stream generator, use `into` argument defined convert method, use for not has `Into<Vec<u8>>` message.

```rust
use yrs_tokio_macros::yrs_stream;

#[yrs_stream(into=into_data().into())]
pub struct YrsStream(SplitStream<WebSocket>);
```

## #[derive(YrsSink)]

Yrs tokio sink generator.

 ```rust
use yrs_tokio_macros::YrsSink;

#[derive(YrsSink)]
pub struct YrsSink(SplitSink<WebSocket, AxumMessage>);
 ```

## #[yrs_common_test]

Yrs tokio common test unit generator.

> This macro requires users to add a `tokio-tungstenite` 0.26 dependency.

```rust
use yrs_tokio_macros::yrs_common_test;

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
```