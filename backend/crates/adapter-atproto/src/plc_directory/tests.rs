use super::*;
use crate::plc::PlcOperation;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn signed_op() -> crate::plc::SignedOperation {
    PlcOperation::identity_only(
        vec!["did:key:a".to_string(), "did:key:b".to_string()],
        "did:key:c".to_string(),
        "alice.zurfur.app",
    )
    .into_signed("sig".to_string())
}

// The HTTP submitter must (1) trim a trailing slash on the base URL so the
// target path is single-slashed `/{did}`, and (2) surface a non-2xx response as
// an error. A one-shot local server captures the request line and replies 400.
#[tokio::test]
async fn http_directory_trims_trailing_slash_and_errors_on_non_2xx() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let request_line = Arc::new(Mutex::new(String::new()));
    let captured = request_line.clone();

    let server = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 2048];
        let n = sock.read(&mut buf).await.unwrap();
        let req = String::from_utf8_lossy(&buf[..n]);
        *captured.lock().unwrap() = req.lines().next().unwrap_or("").to_string();
        sock.write_all(
            b"HTTP/1.1 400 Bad Request\r\ncontent-length: 3\r\nconnection: close\r\n\r\nbad",
        )
        .await
        .unwrap();
        let _ = sock.shutdown().await;
    });

    // Base URL carries a trailing slash on purpose — it must be trimmed.
    let dir = HttpPlcDirectory::new(format!("http://{addr}/"));
    let res = dir
        .submit("did:plc:x", &signed_op().to_json().unwrap())
        .await;

    assert!(res.is_err(), "a non-2xx response must be an error");
    server.await.unwrap();
    let line = request_line.lock().unwrap().clone();
    assert!(
        line.starts_with("POST /did:plc:x "),
        "path must be single-slashed `/did:plc:x`, got request line: {line:?}"
    );
}
