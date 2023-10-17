use std::time::Duration;

use anyhow::{anyhow, Result};
use futures::{stream::FuturesUnordered, TryStreamExt as _};
use hyper::{body::to_bytes, Body, Client, Method, Request, Version};
use tower::{timeout::Timeout, Service as _};

use stilsoft_common::call_timing::CallTimedService;

#[tokio::main]
async fn main() -> Result<()> {
    let nreqs: u32 = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("nreqs not given"))?
        .parse()?;

    let mut client = CallTimedService::new(Timeout::new(
        Client::builder().http2_only(true).build_http::<Body>(),
        Duration::from_secs(2),
    ));

    let mut futs: FuturesUnordered<_> = (1..=nreqs).map(|i| client.call(mk_req(i))).collect();

    while let Some(res) = futs.try_next().await.map_err(|e| anyhow!(e))? {
        println!("{}", to_bytes(res.into_body()).await?.escape_ascii());
    }

    Ok(())
}

fn mk_req(i: u32) -> Request<Body> {
    Request::builder()
        .version(Version::HTTP_2)
        .method(Method::GET)
        .uri(format!("http://localhost:8080/{}", i))
        .body(Body::default())
        .unwrap()
}
