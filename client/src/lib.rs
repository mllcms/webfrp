use std::{io, net::SocketAddr, sync::Arc};

use common::{config::Config, connect::Connect, duplex, message::Message};
use derive_more::derive::Deref;
use tokio::{io::AsyncWriteExt, net::TcpStream};

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
        Message::Master(self.secret.clone()).send(&mut connect.tcp).await?;

        println!("│{:21?}│ ClientConnect", self.client_addr);
        println!("│{:21?}│ MasterConnect", self.server_addr);

        let mut addr: Arc<SocketAddr> = Arc::new("0.0.0.0:65535".parse().unwrap());
        let mut message = connect.listen_message("Master".to_string(), self.heartbeat).await;

        while let Some(msg) = message.recv().await {
            match msg {
                Message::New => new_worker(self.clone(), addr.clone()).await,
                Message::Msg(msg) => println!("{msg}"),
                Message::Worker(a) => {
                    eprintln!("│{:21?}│ WorkerConnect", a);
                    addr = Arc::new(a)
                }
                Message::Error(err) => return Ok(eprintln!("{err}")),
                _ => {}
            }
        }
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
            Ok(local) => duplex(remote, local, client.timeout),
            Err(err) => {
                remote.write_all(Client::NO_CLIENT).await.ok();
                remote.flush().await.ok();
                eprintln!("Client Connect Error: {err}")
            }
        }
    });
}
