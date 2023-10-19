use std::{
    fmt::Display,
    sync::{Arc, Mutex},
    task,
    time::{Duration, Instant},
};

use futures::future::BoxFuture;
use humantime::format_duration;
use tower::Service;

#[derive(Debug)]
struct SomeCallTiming {
    number: u32,
    min: Duration,
    max: Duration,
    sum: Duration,
}

#[derive(Default)]
pub struct CallTiming(Option<SomeCallTiming>);

impl CallTiming {
    pub fn add(&mut self, duration: Duration) {
        if let Some(timing) = &mut self.0 {
            timing.number += 1;
            timing.min = timing.min.min(duration);
            timing.max = timing.max.max(duration);
            timing.sum += duration;
        } else {
            self.0 = Some(SomeCallTiming {
                number: 1,
                min: duration,
                max: duration,
                sum: duration,
            });
        }
    }
}

impl Display for CallTiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(timing) = &self.0 {
            write!(
                f,
                "number={}, min={}, max={}, avg={}",
                timing.number,
                format_duration(timing.min),
                format_duration(timing.max),
                format_duration(timing.sum / timing.number),
            )
        } else {
            write!(f, "number=0")
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
            call_timing: Arc::new(Mutex::new(Default::default())),
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
