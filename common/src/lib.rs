pub mod config;
pub mod connect;
pub mod message;

use std::{
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    task::JoinSet,
};

/// 全双工通信
pub fn duplex(a: TcpStream, b: TcpStream, timeout: Duration) {
    let (fr, fw) = a.into_split();
    let (tr, tw) = b.into_split();
    let mut join_set = JoinSet::new();
    let count = Arc::new(AtomicU64::new(0));
    join_set.spawn(forward(fr, tw, Some(count.clone())));
    join_set.spawn(forward(tr, fw, Some(count.clone())));

    let mut prev = 0;
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = join_set.join_next() => return,
                _ = tokio::time::sleep(timeout) => {
                    let curr = count.load(Ordering::Relaxed);
                    if prev == curr {
                        return eprintln!("Duplex Communication Timeout")
                    }
                    prev = curr
                }
            }
        }
    });
}

/// 数据转发
pub async fn forward<R, W>(mut reader: R, mut writer: W, count: Option<Arc<AtomicU64>>) -> io::Result<()>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let mut buf = [0; 1024];
    if let Some(c) = count {
        while let Ok(n) = reader.read(&mut buf).await {
            if n == 0 {
                break;
            }
            c.fetch_add(1, Ordering::Relaxed);
            writer.write_all(&buf[..n]).await?;
        }
    } else {
        while let Ok(n) = reader.read(&mut buf).await {
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n]).await?;
        }
    }
    writer.shutdown().await
}
