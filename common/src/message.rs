use std::{io, net::SocketAddr};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Message {
    New,
    Ping,
    Pong,
    Msg(String),
    Error(String),
    Master(String),
    Worker(SocketAddr),
}

impl Message {
    pub fn from_buf(buf: &[u8]) -> io::Result<Self> {
        serde_json::from_slice(buf).map_err(|err| io::Error::other(format!("序列化失败: {err}")))
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = serde_json::to_vec(self).unwrap();
        vec.push(b'\n');
        vec
    }

    pub async fn send<T>(&self, writer: &mut T) -> io::Result<()>
    where
        T: AsyncWriteExt + Unpin,
    {
        let src = self.to_vec();
        writer.write_all(&src).await?;
        writer.flush().await
    }
}
