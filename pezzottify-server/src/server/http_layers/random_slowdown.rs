//! Random slowdown middleware for testing
#![allow(dead_code)] // Feature-gated middleware

use rand_distr::{Distribution, Normal};
use simple_server::axum::body::Body;
use simple_server::axum::extract::Request;
use simple_server::axum::middleware::Next;
use simple_server::axum::response::IntoResponse;

/// Middleware that slows down the request for a random amount of time.
/// The random amount of time is a gaussian distribution with a mean of 2 seconds and a standard deviation of 1 second.
pub async fn slowdown_request(request: Request<Body>, next: Next) -> impl IntoResponse {
    // mean 2, standard deviation 3
    let normal = Normal::new(1000.0, 2000.0).unwrap();
    let v = 0.0f64.max(normal.sample(&mut rand::rng()));

    std::thread::sleep(std::time::Duration::from_millis(v as u64));
    next.run(request).await
}
