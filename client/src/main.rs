use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use futures::{stream::FuturesUnordered, TryStreamExt as _};
use hyper::{body::to_bytes, Body, Method, Request, Version, client::conn::SendRequest};
use tokio::{net::TcpStream, time::timeout};
use tower::{Service as _, ServiceExt as _};

use stilsoft_common::call_timing::CallTimedService;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let nreqs = cli.nreqs;

    let mut client = CallTimedService::new(
        timeout(Duration::from_secs(2), connect()).await??
    );

    let mut futs = FuturesUnordered::new();

    for i in 1..=nreqs {
        let req = mk_req(i);
        client.ready().await?;
        futs.push(client.call(req));
    }

    while let Some(res) = futs.try_next().await? {
        println!("{}", to_bytes(res.into_body()).await?.escape_ascii());
    }

    Ok(())
}

async fn connect() -> Result<SendRequest<Body>> {
    let stream = TcpStream::connect("localhost:8080").await?;
    let (sender, connection) = hyper::client::conn::Builder::new()
        .http2_only(true)
        .handshake(stream)
        .await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Error in connection: {}", e);
        }
    });
    Ok(sender)
}

fn mk_req(i: u32) -> Request<Body> {
    Request::builder()
        .version(Version::HTTP_2)
        .method(Method::GET)
        .uri(format!("http://localhost:8080/{}", i))
        .body(Body::default())
        .unwrap()
}

#[derive(Parser)]
struct Cli {
    #[arg(long, help = "number of requests to make", value_parser=1..=100)]
    nreqs: u32,
}
