pub mod config;
pub mod connect;
pub mod message;

use std::io;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// 全双工通信
pub fn duplex(a: TcpStream, b: TcpStream) {
    let (fr, fw) = a.into_split();
    let (tr, tw) = b.into_split();
    tokio::spawn(forward(fr, tw));
    tokio::spawn(forward(tr, fw));
}

/// 数据转发
pub async fn forward<R, W>(mut reader: R, mut writer: W) -> io::Result<()>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let mut buf = [0; 1024];
    while let Ok(n) = reader.read(&mut buf).await {
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n]).await?;
    }
    writer.shutdown().await
}
