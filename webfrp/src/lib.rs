use std::{io, sync::Arc};

use async_channel::{unbounded, Receiver, Sender};
use common::{config::Config, connect::Connect, message::Message};
use derive_more::derive::Deref;
use salvo::{
    conn::{Accepted, Acceptor, Holding, SocketAddr, StraightStream},
    fuse::{FuseFactory, FuseInfo, TransProto},
    http::{uri::Scheme, Version},
    Listener,
};
use tokio::net::TcpStream;

#[derive(Deref)]
pub struct FrpListen {
    #[deref]
    config: Arc<Config>,
    holdings: Vec<Holding>,
    tx: Sender<(TcpStream, SocketAddr)>,
    rx: Receiver<(TcpStream, SocketAddr)>,
}

impl Acceptor for FrpListen {
    type Conn = StraightStream<TcpStream>;

    #[inline]
    fn holdings(&self) -> &[Holding] {
        &self.holdings
    }

    #[inline]
    async fn accept(
        &mut self,
        fuse_factory: Option<Arc<dyn FuseFactory + Sync + Send + 'static>>,
    ) -> io::Result<Accepted<Self::Conn>> {
        self.rx
            .recv()
            .await
            .map(move |(conn, remote_addr)| {
                let local_addr = self.holdings[0].local_addr.clone();
                let remote_addr2 = remote_addr.clone();
                let conn = StraightStream::new(
                    conn,
                    fuse_factory.map(|f| {
                        f.create(FuseInfo {
                            trans_proto: TransProto::Tcp,
                            remote_addr,
                            local_addr: local_addr.clone(),
                        })
                    }),
                );
                Accepted {
                    conn,
                    remote_addr: remote_addr2,
                    local_addr,
                    http_scheme: Scheme::HTTP,
                }
            })
            .map_err(io::Error::other)
    }
}

impl FrpListen {
    pub async fn new(config: Config) -> Self {
        let (tx, rx) = unbounded();
        let addr = config.server_addr;
        let frp = Self {
            tx,
            rx,
            holdings: vec![Holding {
                local_addr: addr.into(),
                http_versions: vec![Version::HTTP_11],
                http_scheme: Scheme::HTTP,
            }],
            config: Arc::new(config),
        };
        frp.run().await.unwrap();
        frp
    }

    pub async fn run(&self) -> io::Result<()> {
        let tx = self.tx.clone();
        let mut connect = Connect::connect(self.server_addr, self.timeout).await?;
        Message::Master(self.secret.clone()).send(&mut connect.tcp).await?;

        println!("│{:21?}│ ClientConnect", self.client_addr);
        println!("│{:21?}│ MasterConnect", self.server_addr);

        let mut message = connect.listen_message("Master".to_string(), self.heartbeat).await;
        tokio::spawn(async move {
            let mut addr: Arc<std::net::SocketAddr> = Arc::new("0.0.0.0:65535".parse().unwrap());
            while let Some(msg) = message.recv().await {
                match msg {
                    Message::New => new_worker(addr.clone(), tx.clone()),
                    Message::Msg(msg) => println!("{msg}"),
                    Message::Worker(a) => {
                        eprintln!("│{:21?}│ WorkerConnect", a);
                        addr = Arc::new(a)
                    }
                    Message::Error(err) => return eprintln!("{err}"),
                    _ => {}
                }
            }
        });
        Ok(())
    }
}

impl Listener for FrpListen {
    type Acceptor = Self;

    async fn try_bind(self) -> salvo_core::Result<Self::Acceptor> {
        Ok(self)
    }
}

impl Unpin for FrpListen {}

pub fn new_worker(addr: Arc<std::net::SocketAddr>, tx: Sender<(TcpStream, SocketAddr)>) {
    tokio::spawn(async move {
        let remote = match TcpStream::connect(*addr).await {
            Ok(v) => v,
            Err(err) => return eprintln!("Worker Connect Error: {err}"),
        };
        tx.send((remote, (*addr).into())).await.unwrap()
    });
}
