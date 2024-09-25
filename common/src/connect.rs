use std::{
    future::Future,
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
    task::JoinSet,
};

use crate::message::Message;

pub struct Connect {
    pub tcp: TcpStream,
    pub addr: SocketAddr,
    pub instant: Instant,
}

impl From<(TcpStream, SocketAddr)> for Connect {
    fn from((tcp, addr): (TcpStream, SocketAddr)) -> Self {
        Self::new(tcp, addr)
    }
}

impl Connect {
    pub fn new(tcp: TcpStream, addr: SocketAddr) -> Self {
        let instant = Instant::now();
        Self { tcp, addr, instant }
    }

    pub async fn connect(addr: SocketAddr, timeout: Duration) -> io::Result<Self> {
        tokio::select! {
            tcp = TcpStream::connect(&addr) => Ok(Connect::new(tcp?,addr)),
            _ = tokio::time::sleep(timeout) => Err(io::Error::other("Connect Timeout"))
        }
    }

    pub fn is_timeout(&self, timeout: Duration) -> bool {
        self.instant.elapsed() > timeout
    }

    pub async fn from_tcp(tcp: &TcpListener) -> io::Result<Self> {
        tcp.accept().await.map(Self::from)
    }

    pub async fn send(&mut self, message: &Message) -> io::Result<()> {
        message.send(&mut self.tcp).await
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.tcp.read(buf).await
    }

    pub async fn write(&mut self, buf: &[u8]) -> io::Result<()> {
        self.tcp.write_all(buf).await?;
        self.tcp.flush().await
    }

    /// 读写分离
    pub async fn split<R, W>(self, id: String, r: impl FnOnce(OwnedReadHalf) -> R, w: impl FnOnce(OwnedWriteHalf) -> W)
    where
        R: Future<Output = io::Result<()>> + Send + 'static,
        W: Future<Output = io::Result<()>> + Send + 'static,
    {
        let (reader, writer) = self.tcp.into_split();
        let mut join_set = JoinSet::new();
        join_set.spawn(r(reader));
        join_set.spawn(w(writer));
        if let Some(Ok(Err(err))) = join_set.join_next().await {
            eprintln!("│{:21?}│ {id} {err}", self.addr)
        }
    }

    pub async fn listen_message(self, id: String, heartbeat: Duration) -> UnboundedReceiver<Message> {
        let (tx, rx) = unbounded_channel();
        let name = id.clone();
        let reader = |r| async move {
            let mut reader = BufReader::new(r);
            let mut buf = String::new();

            while let Ok(true) = reader.read_line(&mut buf).await.map(|n| n > 1) {
                match Message::from_buf(buf.as_bytes()) {
                    Err(err) => eprintln!("Serialization Failed:{err} Content:{buf}",),
                    Ok(msg) => tx.send(msg).unwrap(),
                };
                buf.truncate(0)
            }
            Ok(eprintln!("{name} Connect Disconnected"))
        };

        let name = id.clone();
        let writer = move |mut w| async move {
            loop {
                tokio::time::sleep(heartbeat).await;
                if let Err(err) = Message::Pong.send(&mut w).await {
                    return Ok(eprintln!("{name} Connect Disconnected: {err}"));
                };
            }
        };

        tokio::spawn(self.split(id, reader, writer));
        rx
    }
}
