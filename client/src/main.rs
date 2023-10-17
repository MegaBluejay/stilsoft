use anyhow::Result;
use futures::{stream::FuturesUnordered, TryStreamExt as _};
use hyper::{body::to_bytes, Body, Client, Method, Request, Version};
use tower::Service as _;

use stilsoft_common::call_timing::CallTimedService;

#[tokio::main]
async fn main() -> Result<()> {
    let mut client = CallTimedService::new(Client::builder().http2_only(true).build_http::<Body>());

    let mut futs: FuturesUnordered<_> = (1..10).map(|i| client.call(mk_req(i))).collect();

    while let Some(res) = futs.try_next().await? {
        println!(
            "{}: {}",
            std::env::args().collect::<Vec<_>>()[1],
            to_bytes(res.into_body()).await?.escape_ascii()
        );
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
