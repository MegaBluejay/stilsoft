use std::{
    convert::Infallible,
    io::ErrorKind,
    net::SocketAddr,
    process::exit,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;
use clap::Parser;
use humantime::format_duration;
use hyper::{server::conn::Http, Body, Request, Response};
use rand::{thread_rng, Rng as _};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
};
use tower::{service_fn, Service as _};

use stilsoft_common::call_timing::CallTimedService;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let listener = TcpListener::bind(cli.addr).await?;
    let semaphore = Arc::new(Semaphore::new(5));

    let broken_pipes = Arc::new(Mutex::new(0));
    let mut connection_handler = CallTimedService::new(service_fn(handle_conn));

    let ctrlc_broken_pipes = broken_pipes.clone();
    let ctrlc_conn_timing = connection_handler.call_timing.clone();
    ctrlc::set_handler(move || {
        println!(
            "connections: {}, broken pipes: {}",
            ctrlc_conn_timing.lock().unwrap(),
            ctrlc_broken_pipes.lock().unwrap()
        );
        exit(0);
    })?;

    loop {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let (stream, _) = listener.accept().await?;
        tokio::task::spawn(connection_handler.call((permit, stream, broken_pipes.clone())));
    }
}

async fn handle_conn(
    (permit, mut stream, broken_pipes): (OwnedSemaphorePermit, TcpStream, Arc<Mutex<u32>>),
) -> Result<()> {
    let start = Instant::now();
    let mut request_handler = CallTimedService::new(service_fn(handle_request));

    if let Err(err) = Http::new()
        .http2_only(true)
        .serve_connection(&mut stream, &mut request_handler)
        .await
    {
        eprintln!("Error serving: {}", err);
        if let Some(cause) = err.into_cause() {
            if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
                if io_err.kind() == ErrorKind::BrokenPipe {
                    *broken_pipes.lock().unwrap() += 1;
                }
            }
        }
    }
    drop(stream);
    drop(permit);

    println!(
        "session: {}, requests: {}",
        format_duration(start.elapsed()),
        request_handler.call_timing.lock().unwrap()
    );
    Ok(())
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let delay = thread_rng().gen_range(100..=500);
    tokio::time::sleep(Duration::from_millis(delay)).await;
    Ok(Response::new(Body::from(req.uri().path().to_owned())))
}

#[derive(Parser)]
struct Cli {
    #[arg(long, help = "socket address, e.g. 127.0.0.1:8080")]
    addr: SocketAddr,
}
