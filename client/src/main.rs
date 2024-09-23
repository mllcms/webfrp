use std::env;

use client::Client;
use common::config::Config;

#[tokio::main]
async fn main() {
    let path = env::args().nth(1).unwrap_or("config.toml".to_string());
    let config = Config::load(&path);
    Client::new(config).run().await.unwrap()
}
