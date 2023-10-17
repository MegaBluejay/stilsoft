use std::{
    convert::Infallible,
    fmt::Display,
    io::ErrorKind,
    net::SocketAddr,
    process::exit,
    sync::{Arc, Mutex},
    task,
    time::{Duration, Instant},
};

use anyhow::Result;
use futures::future::BoxFuture;
use humantime::format_duration;
use hyper::{server::conn::Http, Body, Request, Response};
use rand::{thread_rng, Rng};
use smart_default::SmartDefault;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
};
use tower::{service_fn, Service};

#[tokio::main]
async fn main() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let listener = TcpListener::bind(addr).await?;
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

#[derive(SmartDefault, Debug)]
struct CallTiming {
    #[default(0)]
    number: u32,
    #[default(Duration::MAX)]
    min: Duration,
    #[default(Duration::ZERO)]
    max: Duration,
    #[default(Duration::ZERO)]
    sum: Duration,
}

impl CallTiming {
    fn add(&mut self, duration: Duration) {
        self.number += 1;
        self.min = self.min.min(duration);
        self.max = self.max.max(duration);
        self.sum += duration;
    }
}

impl Display for CallTiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.number == 0 {
            write!(f, "number=0")
        } else {
            write!(
                f,
                "number={}, min={}, max={}, avg={}",
                self.number,
                format_duration(self.min),
                format_duration(self.max),
                format_duration(self.sum / self.number),
            )
        }
    }
}

struct CallTimedService<S> {
    call_timing: Arc<Mutex<CallTiming>>,
    inner: S,
}

impl<S> CallTimedService<S> {
    fn new(inner: S) -> Self {
        Self {
            call_timing: Arc::new(Mutex::new(CallTiming::default())),
            inner,
        }
    }
}

impl<Request, S> Service<Request> for CallTimedService<S>
where
    S: Service<Request>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let fut = self.inner.call(req);
        let timing = self.call_timing.clone();
        Box::pin(async move {
            let start = Instant::now();
            let res = fut.await;
            let elapsed = start.elapsed();
            timing.lock().unwrap().add(elapsed);
            res
        })
    }
}
