#![forbid(unsafe_code)]
#![warn(unused_crate_dependencies)]

mod auth;
mod config;
mod handler;
mod user;

pub use std::error::Error as StdError;
use std::{net::SocketAddr, sync::Arc};

use libunftp::Server;

use crate::{
    auth::Authenticator,
    handler::Filesystem,
    user::{User, UserMap},
};

pub type BoxedStdError = Box<dyn StdError>;

fn main() -> Result<(), BoxedStdError> {
    let users = config::load("config.yaml")?;

    pretty_env_logger::init();

    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(run(Arc::new(users)))?;

    Ok(())
}

async fn run(users: Arc<UserMap>) -> Result<(), BoxedStdError> {
    let server: Server<Filesystem, User> = Server::with_authenticator(
        Box::new(move || Filesystem),
        Arc::new(Authenticator {
            users: Arc::clone(&users),
        }),
    )
    .greeting("Welcome to my FTP server")
    .passive_ports(60000..65535);

    let addr: SocketAddr = SocketAddr::from(([0, 0, 0, 0], 2121));

    eprintln!("Starting on {addr}");
    server.listen(addr.to_string()).await?;
    eprintln!("Goodbye!");

    Ok(())
}
