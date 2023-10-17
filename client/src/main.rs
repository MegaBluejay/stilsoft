use anyhow::Result;
use futures::{stream::FuturesUnordered, TryStreamExt};
use hyper::{Body, Client, body::to_bytes, Request, Method, Version};

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::builder().http2_only(true).build_http::<Body>();

    let mut futs: FuturesUnordered<_> = (1..10).map(|i| client.request(mk_req(i))).collect();

    while let Some(res) = futs.try_next().await? {
        println!("{}: {}", std::env::args().collect::<Vec<_>>()[1], to_bytes(res.into_body()).await?.escape_ascii());
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
