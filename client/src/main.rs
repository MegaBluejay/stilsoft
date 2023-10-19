use std::time::Duration;

use anyhow::Result;
use clap::clap_app;
use futures::{stream::FuturesUnordered, TryStreamExt as _};
use hyper::{body::to_bytes, client::conn::SendRequest, Body, Method, Request, Version};
use tokio::{net::TcpStream, time::timeout};
use tower::{Service as _, ServiceExt as _};

use stilsoft_common::call_timing::CallTimedService;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = clap_app!(stilsoft_client =>
        (@arg nreqs: --nreqs +takes_value +required "number of requests to make")
    )
    .get_matches();
    let nreqs: u32 = matches.value_of("nreqs").unwrap().parse()?;

    let mut client = CallTimedService::new(timeout(Duration::from_secs(2), connect()).await??);

    let mut futs = FuturesUnordered::new();

    for i in 1..=nreqs {
        let req = mk_req(i);
        client.ready().await?;
        futs.push(client.call(req));
    }

    while let Some(res) = futs.try_next().await? {
        println!(
            "{}",
            String::from_utf8_lossy(&to_bytes(res.into_body()).await?)
        );
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
