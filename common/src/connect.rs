use std::{
    future::Future,
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
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
    /// 传入 join_set 第一个任务退出所有任务都退出
    pub async fn split<R, W>(
        self,
        name: &str,
        mut join_set: JoinSet<io::Result<()>>,
        r: impl FnOnce(OwnedReadHalf) -> R,
        w: impl FnOnce(OwnedWriteHalf) -> W,
    ) where
        R: Future<Output = io::Result<()>> + Send + 'static,
        W: Future<Output = io::Result<()>> + Send + 'static,
    {
        let (reader, writer) = self.tcp.into_split();
        join_set.spawn(r(reader));
        join_set.spawn(w(writer));
        if let Some(Err(err)) = join_set.join_next().await {
            println!("│{:21?}│ {name} {err}", self.addr)
        }
    }
}
