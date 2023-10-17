use std::{
    fmt::Display,
    sync::{Arc, Mutex},
    task,
    time::{Duration, Instant},
};

use futures::future::BoxFuture;
use humantime::format_duration;
use smart_default::SmartDefault;
use tower::Service;

#[derive(SmartDefault, Debug)]
pub struct CallTiming {
    #[default(0)]
    pub number: u32,
    #[default(Duration::MAX)]
    pub min: Duration,
    #[default(Duration::ZERO)]
    pub max: Duration,
    #[default(Duration::ZERO)]
    pub sum: Duration,
}

impl CallTiming {
    pub fn add(&mut self, duration: Duration) {
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

pub struct CallTimedService<S> {
    pub call_timing: Arc<Mutex<CallTiming>>,
    pub inner: S,
}

impl<S> CallTimedService<S> {
    pub fn new(inner: S) -> Self {
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
