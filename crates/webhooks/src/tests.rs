use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::task::JoinHandle;

pub(crate) struct TestListener {
    handle: JoinHandle<()>,
    pub rx: Receiver<u8>,
}
impl Drop for TestListener {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub(crate) async fn get_test_listener(port: u16, status_code: u16) -> TestListener {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    let (tx, rx) = channel(10);
    let handle = tokio::spawn(listener_task(status_code, listener, tx));
    TestListener { handle, rx }
}

async fn listener_task(status_code: u16, listener: TcpListener, tx: Sender<u8>) {
    let mut buf = [0; 4096];

    loop {
        let (mut stream, _) = listener.accept().await.unwrap();
        loop {
            stream.readable().await.unwrap();

            match stream.try_read(&mut buf) {
                Ok(_) => tx.send(0).await.unwrap(),
                Err(_) => break,
            }
        }

        stream.writable().await.unwrap();
        stream
            .write_all(format!("HTTP/1.1 {status_code}\r\nContent-Length: 0\r\n\r\n").as_bytes())
            .await
            .unwrap();
    }
}
