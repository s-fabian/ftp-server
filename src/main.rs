#![forbid(unsafe_code)]
#![warn(unused_crate_dependencies)]

mod auth;
mod config;
mod handler;
mod user;

pub use std::error::Error as StdError;
use std::{net::SocketAddr, sync::Arc};

use libunftp::Server;
use sha3::{Digest, Sha3_256};

use crate::{
    auth::Authenticator,
    handler::Filesystem,
    user::{User, UserMap},
};

pub type BoxedStdError = Box<dyn StdError>;


fn hash(pw: impl AsRef<str>) -> String {
    // create a SHA3-256 object
    let mut hasher = Sha3_256::new();

    // write input message
    hasher.update(pw.as_ref().as_bytes());

    // read hash digest
    let result = hasher.finalize();

    // format as hex
    format!("{:x}", result)
}


fn main() -> Result<(), BoxedStdError> {
    let mut args = std::env::args().skip(1);
    if args.next().is_some_and(|s| s == "sha3") {
        let pw: String = args.collect::<Vec<String>>().join(" ");

        if pw.is_empty() {
            return Err("Error: no password provided".into());
        }

        let pw = hash(pw);

        println!("Password is {pw}");

        return Ok(());
    }

    let users = config::load(
        std::env::var("FTP_CONFIG").unwrap_or(String::from("./config.yaml")),
    )?;

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
