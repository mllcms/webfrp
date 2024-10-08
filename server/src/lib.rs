use std::{net::SocketAddr, sync::Arc};

use async_channel::{unbounded, Receiver, Sender};
use common::{config::Config, connect::Connect, duplex, message::Message};
use derive_more::derive::Deref;
use tokio::{
    io::{self, AsyncReadExt},
    net::{tcp::OwnedReadHalf, TcpListener},
    time::sleep,
};

#[derive(Deref)]
pub struct ServerInner {
    #[deref]
    config: Config,
    master_tx: Sender<Message>,
    master_rx: Receiver<Message>,
    accept_tx: Sender<Connect>,
    accept_rx: Receiver<Connect>,
}

#[derive(Clone, Deref)]
pub struct Server(Arc<ServerInner>);

impl Server {
    pub const NO_MASTER: &'static [u8; 30] = b"HTTP/1.1 555 No Master Connect";

    pub fn new(config: Config) -> Self {
        let (accept_tx, accept_rx) = unbounded();
        let (master_tx, master_rx) = unbounded();
        Self(Arc::new(ServerInner {
            config,
            master_tx,
            master_rx,
            accept_rx,
            accept_tx,
        }))
    }
    pub async fn run(&self) -> io::Result<()> {
        let ml = TcpListener::bind(&self.server_addr).await?;
        let al = TcpListener::bind(&self.accept_addr).await?;
        println!("│{:21?}│ AcceptListen", self.accept_addr);
        println!("│{:21?}│ MasterLister", self.server_addr);

        loop {
            tokio::select! {
                tcp = ml.accept() => tokio::spawn(master(self.clone(),tcp?.into())),
                tcp = al.accept() => tokio::spawn(accept(self.clone(),tcp?.into())),
            };
        }
    }
}

/// 处理连接
pub async fn master(server: Server, mut connect: Connect) {
    if server.master_rx.receiver_count() > 2 {
        let msg = Message::Error("Master already exists".to_string());
        msg.send(&mut connect.tcp).await.ok();
        return;
    }

    let mut buf = [0; 256];
    let n = connect.read(&mut buf).await.unwrap();

    match serde_json::from_slice::<Message>(&buf[..n]) {
        Ok(Message::Master(secret)) if secret == server.secret => {
            let (addr, listen) = new_worker(server.clone()).await.unwrap();
            connect.send(&Message::Worker(addr)).await.ok();
            println!("│{:21?}│ WorkerListen", addr);
            println!("│{:21?}│ ⇦ Master", connect.addr);

            let handle = tokio::spawn(run_worker(server.clone(), listen));

            let reader = |mut r: OwnedReadHalf| async move {
                let mut buf = [0; 256];
                while let Ok(true) = r.read(&mut buf).await.map(|n| n > 1) {}
                handle.abort();
                Err(io::Error::other("Connect Disconnected"))
            };

            let writer = |mut w| async move {
                let master_rx = server.master_rx.clone();
                loop {
                    tokio::select! {
                        Ok(msg) = master_rx.recv() => msg.send(&mut w).await?,
                        _ = sleep(server.heartbeat) => Message::Ping.send(&mut w).await?
                    }
                }
            };

            tokio::spawn(connect.split("⇨ Master".to_string(), reader, writer));
        }
        _ => {
            let msg = Message::Error("Master Secret Error".to_string());
            connect.send(&msg).await.ok();
        }
    }
}

/// 处理访问
pub async fn accept(server: Server, mut connect: Connect) {
    match server.master_rx.receiver_count() {
        1 => {
            println!("│{:21?}│ ⇨ Accept Ready But No Master", connect.addr);
            connect.write(Server::NO_MASTER).await.ok();
        }
        2 => {
            server.master_tx.send(Message::New).await.unwrap();
            server.accept_tx.send(connect).await.unwrap();
        }
        _ => panic!("Accept Failed Master Count Exception"),
    };
}

async fn new_worker(server: Server) -> io::Result<(SocketAddr, TcpListener)> {
    for port in 0xAAAA..0xFFFF {
        let addr = SocketAddr::new(server.server_addr.ip(), port);
        if let Ok(listen) = TcpListener::bind(addr).await {
            return Ok((addr, listen));
        }
    }
    Err(io::Error::other("Listen Worker Failed"))
}

async fn run_worker(server: Server, listen: TcpListener) -> io::Result<()> {
    'l: loop {
        let server = server.clone();
        let (tcp, addr) = listen.accept().await?;
        while let Ok(from) = server.accept_rx.try_recv() {
            if from.is_timeout(server.timeout) {
                eprintln!("│{:21?}│ ⇨ Accept Wait Timeout", from.addr)
            } else {
                duplex(from.tcp, tcp, server.timeout);
                continue 'l;
            }
        }
        eprintln!("│{:21?}│ ⇨ Worker Ready But No Accept", addr)
    }
}
