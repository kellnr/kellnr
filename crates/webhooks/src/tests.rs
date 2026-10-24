use std::sync::{Arc, Mutex};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::task::JoinHandle;

pub(crate) struct TestListener {
    handle: JoinHandle<()>,
    pub rx: Receiver<u8>,
    /// All raw request data received by the listener.
    pub received: Arc<Mutex<String>>,
}
impl Drop for TestListener {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub(crate) async fn get_test_listener(port: u16, status_code: u16) -> TestListener {
    get_test_listener_with_response(
        port,
        format!("HTTP/1.1 {status_code}\r\nContent-Length: 0\r\n\r\n"),
    )
    .await
}

pub(crate) async fn get_redirect_listener(port: u16, location: &str) -> TestListener {
    get_test_listener_with_response(
        port,
        format!("HTTP/1.1 302\r\nLocation: {location}\r\nContent-Length: 0\r\n\r\n"),
    )
    .await
}

pub(crate) async fn get_test_listener_with_response(port: u16, response: String) -> TestListener {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    let (tx, rx) = channel(10);
    let received = Arc::new(Mutex::new(String::new()));
    let handle = tokio::spawn(listener_task(response, listener, tx, received.clone()));
    TestListener {
        handle,
        rx,
        received,
    }
}

async fn listener_task(
    response: String,
    listener: TcpListener,
    tx: Sender<u8>,
    received: Arc<Mutex<String>>,
) {
    let mut buf = [0; 4096];

    loop {
        let (mut stream, _) = listener.accept().await.unwrap();
        loop {
            stream.readable().await.unwrap();

            match stream.try_read(&mut buf) {
                Ok(n) => {
                    received
                        .lock()
                        .unwrap()
                        .push_str(&String::from_utf8_lossy(&buf[..n]));
                    tx.send(0).await.unwrap();
                }
                Err(_) => break,
            }
        }

        stream.writable().await.unwrap();
        stream.write_all(response.as_bytes()).await.unwrap();
    }
}
