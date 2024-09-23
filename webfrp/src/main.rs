use common::config::Config;
use salvo::prelude::*;
use salvo_core::Server;
use webfrp::FrpListen;

#[tokio::main]
async fn main() {
    let config = Config::load("config.toml");
    let listen = TcpListener::new(config.client_addr)
        .join(FrpListen::new(config).await)
        .bind()
        .await;
    let dir = StaticDir::new("./dist").auto_list(true).defaults("index.html");
    let router: Router = Router::new()
        .push(Router::new().path("/hello").get(hello))
        .push(Router::with_path("<**path>").get(dir));
    Server::new(listen).serve(router).await
}

#[handler]
async fn hello() -> &'static str {
    "Hello World!"
}
