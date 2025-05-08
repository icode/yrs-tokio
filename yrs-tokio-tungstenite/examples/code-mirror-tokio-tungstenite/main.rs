use futures_util::StreamExt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::accept_async;
use yrs::sync::Awareness;
use yrs::{Doc, Text, Transact};
use yrs_tokio::broadcast::BroadcastGroup;
use yrs_tokio::AwarenessRef;
use yrs_tokio_tungstenite::{YrsSink, YrsStream};


#[tokio::main]
async fn main() {
    let awareness: AwarenessRef = {
        let doc = Doc::new();
        {
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

    let bcast = Arc::new(BroadcastGroup::new(awareness.clone(), 32).await);

    let addr = "0.0.0.0:8000"
        .parse::<SocketAddr>()
        .expect("无法解析监听地址");

    let listener = TcpListener::bind(&addr).await.expect("无法监听指定地址");
    println!("服务器正在监听: {}", addr);

    while let Ok((stream, peer_addr)) = listener.accept().await {
        println!("接受到来自 {} 的连接", peer_addr);

        let bcast_clone = Arc::clone(&bcast);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, bcast_clone).await {
                eprintln!("连接处理失败: {}", e);
            } else {
                println!("连接处理完毕");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    bcast: Arc<BroadcastGroup>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut peek_buf = [0u8; 1024];
    let n = stream.peek(&mut peek_buf).await?;
    let request_str = String::from_utf8_lossy(&peek_buf[..n]);

    let is_websocket_upgrade = request_str.contains("Upgrade: websocket")
        && request_str.contains("Connection: Upgrade")
        && request_str.contains("GET /my-room HTTP");

    if is_websocket_upgrade {
        println!("检测到 WebSocket 请求，尝试握手...");
        let ws_stream = accept_async(stream).await?;
        println!("WebSocket 握手成功");
        peer(ws_stream, bcast).await;
        Ok(())
    } else {
        println!("检测到 HTTP 请求，处理静态文件");
        serve_static_file(stream, request_str.to_string()).await?;
        Ok(())
    }
}

async fn serve_static_file(
    mut stream: TcpStream,
    request_str: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = request_str.split_whitespace();
    let method = parts.next();
    let requested_path = parts.next().unwrap_or("/");

    if method != Some("GET") {
        let response = "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }
    
    let static_files_dir: String = format!("{}/../examples/code-mirror/frontend/dist", env!("CARGO_MANIFEST_DIR"));

    let base_dir = PathBuf::from(static_files_dir);
    let requested_relative_path = PathBuf::from(requested_path.trim_start_matches('/'));

    if requested_relative_path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    let mut full_path = base_dir.join(&requested_relative_path);

    let file_to_serve = match fs::metadata(&full_path).await {
        Ok(metadata) => {
            if metadata.is_dir() {
                full_path.push("index.html");
                if fs::metadata(&full_path)
                    .await
                    .map(|m| m.is_file())
                    .unwrap_or(false)
                {
                    full_path
                } else {
                    base_dir.join(requested_relative_path)
                }
            } else {
                full_path
            }
        }
        Err(_) => base_dir.join(requested_relative_path),
    };

    let file_content = fs::read(&file_to_serve).await.ok();

    if let Some(content) = file_content {
        let content_type = file_to_serve
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext_str| match ext_str.to_lowercase().as_str() {
                "html" => "text/html",
                "htm" => "text/html",
                "js" => "text/javascript",
                "css" => "text/css",
                "json" => "application/json",
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "svg" => "image/svg+xml",
                "pdf" => "application/pdf",
                _ => "application/octet-stream",
            })
            .unwrap_or("application/octet-stream");

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n",
            content_type,
            content.len()
        );
        stream.write_all(response.as_bytes()).await?;
        // 一次性输出容易导致链接被重置
        for chunk in content.chunks(1024) {
            stream.write_all(chunk).await?;
            stream.flush().await?;
        }
        stream.shutdown().await?;
    } else {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
    }

    Ok(())
}

async fn peer<T>(ws: tokio_tungstenite::WebSocketStream<T>, bcast: Arc<BroadcastGroup>)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (sink, stream) = ws.split();
    let sink = Arc::new(Mutex::new(YrsSink::from(sink)));
    let stream = YrsStream::from(stream);
    let sub = bcast.subscribe(sink, stream);

    match sub.completed().await {
        Ok(_) => println!("广播通道完成成功"),
        Err(e) => eprintln!("广播通道异常结束: {}", e),
    }
}
