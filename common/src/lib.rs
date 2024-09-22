pub mod config;
pub mod message;

use tokio::{io, net::TcpStream};

pub fn forward(from: TcpStream, to: TcpStream) {
    tokio::spawn(async move {
        let (mut fr, mut fw) = from.into_split();
        let (mut tr, mut tw) = to.into_split();
        tokio::try_join!(io::copy(&mut fr, &mut tw), io::copy(&mut tr, &mut fw))
    });
}
