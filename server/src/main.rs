use std::{net::SocketAddr, convert::Infallible, sync::Arc, time::Duration};

use rand::{thread_rng, Rng};
use anyhow::Result;
use hyper::{server::conn::Http, service::service_fn, Request, Body, Response};
use tokio::{net::{TcpListener, TcpStream}, sync::{Semaphore, OwnedSemaphorePermit}};

#[tokio::main]
async fn main() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let listener = TcpListener::bind(addr).await?;
    let semaphore = Arc::new(Semaphore::new(5));

    loop {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let (stream, _) = listener.accept().await?;
        tokio::task::spawn(handle_conn(permit, stream));
    }
}

async fn handle_conn(permit: OwnedSemaphorePermit, mut stream: TcpStream) {
    if let Err(err) =
        Http::new()
        .http2_only(true)
        .serve_connection(&mut stream, service_fn(handle_request))
        .await {
            eprintln!("Error serving: {}", err);
        }
    drop(stream);
    drop(permit);
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let delay = thread_rng().gen_range(100..=500);
    tokio::time::sleep(Duration::from_millis(delay)).await;
    Ok(Response::new(Body::from(req.uri().path().to_owned())))
}
