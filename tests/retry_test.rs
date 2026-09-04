#[path = "../src/retry.rs"]
mod retry;

use std::time::Duration;

#[test]
fn request_attempts_are_bounded() {
    assert_eq!(retry::MAX_HTTP_ATTEMPTS, 5);
}

#[test]
fn retries_rate_limits_and_transient_server_errors() {
    for status in [408, 425, 429, 500, 502, 503, 504, 599] {
        assert!(retry::is_retryable_status(status), "status {status}");
    }
}

#[test]
fn does_not_retry_permanent_http_errors() {
    for status in [400, 401, 403, 404, 409, 422] {
        assert!(!retry::is_retryable_status(status), "status {status}");
    }
}

#[test]
fn delay_grows_exponentially() {
    assert_eq!(retry::retry_delay(1, None, 0), Duration::from_secs(1));
    assert_eq!(retry::retry_delay(2, None, 0), Duration::from_secs(2));
    assert_eq!(retry::retry_delay(3, None, 0), Duration::from_secs(4));
}

#[test]
fn delay_adds_bounded_jitter() {
    let delay = retry::retry_delay(3, None, u64::MAX);
    assert!(delay >= Duration::from_secs(4));
    assert!(delay <= Duration::from_secs(6));
}

#[test]
fn retry_after_is_honored_and_capped() {
    assert_eq!(retry::retry_delay(1, Some("5"), 0), Duration::from_secs(5));
    assert_eq!(
        retry::retry_delay(1, Some("120"), 0),
        Duration::from_secs(30)
    );
    assert_eq!(
        retry::retry_delay(2, Some("not-a-number"), 0),
        Duration::from_secs(2)
    );
}

#[test]
fn jitter_seed_is_available() {
    let _ = retry::jitter_seed();
}

#[test]
fn native_wait_uses_the_requested_delay() {
    let started = std::time::Instant::now();
    retry::wait(Duration::from_millis(10)).expect("wait should succeed");
    assert!(started.elapsed() >= Duration::from_millis(10));
}
