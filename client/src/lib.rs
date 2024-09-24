use std::{io, net::SocketAddr, sync::Arc};

use common::{config::Config, connect::Connect, duplex, message::Message};
use derive_more::derive::Deref;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    task::JoinSet,
};

#[derive(Deref)]
pub struct ClientInner {
    #[deref]
    config: Config,
}

#[derive(Clone, Deref)]
pub struct Client(Arc<ClientInner>);

impl Client {
    const NO_CLIENT: &'static [u8; 34] = b"HTTP/1.1 555 Client Connect Failed";

    pub fn new(config: Config) -> Self {
        Self(Arc::new(ClientInner { config }))
    }

    pub async fn run(&self) -> io::Result<()> {
        let mut connect = Connect::connect(self.server_addr, self.timeout).await?;

        println!("│{:21?}│ ClientConnect", self.client_addr);
        println!("│{:21?}│ MasterConnect", self.server_addr);

        let secret = Message::Master(self.secret.clone());
        connect.send(&secret).await?;
        let client = self.clone();
        let join_set = JoinSet::new();
        let heartbeat = client.heartbeat;

        let reader = |r| async move {
            let mut reader = BufReader::new(r);
            let mut addr: Arc<SocketAddr> = Arc::new("0.0.0.0:65535".parse().unwrap());
            let mut buf = String::new();

            while let Ok(true) = reader.read_line(&mut buf).await.map(|n| n > 1) {
                match Message::from_buf(buf.as_bytes()) {
                    Err(err) => eprintln!("Serialization Failed:{err} Content:{buf}",),
                    Ok(msg) => match msg {
                        Message::New => new_worker(client.clone(), addr.clone()).await,
                        Message::Msg(msg) => println!("{msg}"),
                        Message::Worker(a) => {
                            eprintln!("│{:21?}│ WorkerConnect", a);
                            addr = Arc::new(a)
                        }
                        Message::Error(err) => return Ok(eprintln!("{err}")),
                        _ => {}
                    },
                };
                buf.truncate(0)
            }
            Ok(eprintln!("Master Connect Disconnected"))
        };

        let writer = |mut w| async move {
            loop {
                tokio::time::sleep(heartbeat).await;
                if let Err(err) = Message::Pong.send(&mut w).await {
                    return Ok(eprintln!("Master Connect Disconnected: {err}"));
                };
            }
        };

        connect.split("Client", join_set, reader, writer).await;
        Ok(())
    }
}

pub async fn new_worker(client: Client, addr: Arc<SocketAddr>) {
    tokio::spawn(async move {
        let mut remote = match TcpStream::connect(*addr).await {
            Ok(v) => v,
            Err(err) => return eprintln!("Worker Connect Error: {err}"),
        };

        match TcpStream::connect(client.client_addr).await {
            Ok(local) => duplex(remote, local),
            Err(err) => {
                remote.write_all(Client::NO_CLIENT).await.ok();
                remote.flush().await.ok();
                eprintln!("Client Connect Error: {err}")
            }
        }
    });
}
