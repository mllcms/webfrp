use std::env;

use common::config::Config;
use server::Server;

#[tokio::main]
async fn main() {
    let path = env::args().nth(1).unwrap_or("config.toml".to_string());
    let config = Config::load(&path);
    Server::new(config).run().await.unwrap()
}
