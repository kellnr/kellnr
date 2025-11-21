use tokio::{
    io::AsyncWriteExt,
    net::TcpListener,
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinHandle,
};

pub(crate) struct TestListener {
    handle: JoinHandle<()>,
    pub rx: Receiver<u8>,
}
impl Drop for TestListener {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub(crate) async fn get_test_listener(port: u16) -> TestListener {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    let (tx, rx) = channel(10);
    let handle = tokio::spawn(listener_task(listener, tx));
    TestListener { handle, rx }
}

async fn listener_task(listener: TcpListener, tx: Sender<u8>) {
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
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
    }
}
