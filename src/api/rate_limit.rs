use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::{
    num::NonZeroU32,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};

/// Rate limiter state using governor
pub type IngestRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Create a new rate limiter for the ingest endpoint
pub fn create_rate_limiter(requests_per_second: u32) -> Arc<IngestRateLimiter> {
    let quota = Quota::per_second(
        NonZeroU32::new(requests_per_second).unwrap_or(NonZeroU32::new(100).unwrap()),
    );
    Arc::new(RateLimiter::direct(quota))
}

/// Layer that applies rate limiting
#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: Arc<IngestRateLimiter>,
}

impl RateLimitLayer {
    pub fn new(limiter: Arc<IngestRateLimiter>) -> Self {
        Self { limiter }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

/// Service that enforces rate limiting
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: Arc<IngestRateLimiter>,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let limiter = self.limiter.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check rate limit
            if limiter.check().is_err() {
                tracing::warn!("Rate limit exceeded for ingest endpoint");
                return Ok(rate_limit_response());
            }

            inner.call(req).await
        })
    }
}

/// Generate a 429 Too Many Requests response
fn rate_limit_response() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [("Retry-After", "1")],
        "Rate limit exceeded. Please retry after 1 second.",
    )
        .into_response()
}
