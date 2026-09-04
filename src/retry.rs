use std::time::Duration;

pub const MAX_HTTP_ATTEMPTS: u32 = 5;

const INITIAL_BACKOFF_MS: u64 = 1_000;
const MAX_BACKOFF_MS: u64 = 30_000;

pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429 | 500..=599)
}

pub fn retry_delay(retry_number: u32, retry_after: Option<&str>, jitter_seed: u64) -> Duration {
    let exponent = retry_number.saturating_sub(1).min(5);
    let exponential_ms = INITIAL_BACKOFF_MS.saturating_mul(1_u64 << exponent);
    let retry_after_ms = retry_after
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000))
        .unwrap_or_default();
    let jitter_ms = jitter_seed % (exponential_ms / 2 + 1);

    Duration::from_millis(
        exponential_ms
            .max(retry_after_ms)
            .saturating_add(jitter_ms)
            .min(MAX_BACKOFF_MS),
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn jitter_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
}

#[cfg(target_arch = "wasm32")]
pub fn jitter_seed() -> u64 {
    let mut bytes = [0_u8; 8];
    if unsafe { wasi::random_get(bytes.as_mut_ptr(), bytes.len()) }.is_ok() {
        u64::from_le_bytes(bytes)
    } else {
        0
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn wait(delay: Duration) -> Result<(), ()> {
    std::thread::sleep(delay);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn wait(delay: Duration) -> Result<(), ()> {
    use std::mem::MaybeUninit;

    let subscription = wasi::Subscription {
        userdata: 0,
        u: wasi::SubscriptionU {
            tag: wasi::EVENTTYPE_CLOCK.raw(),
            u: wasi::SubscriptionUU {
                clock: wasi::SubscriptionClock {
                    id: wasi::CLOCKID_MONOTONIC,
                    timeout: delay.as_nanos().min(u64::MAX as u128) as u64,
                    precision: 0,
                    flags: 0,
                },
            },
        },
    };
    let mut event = MaybeUninit::<wasi::Event>::uninit();

    // The RedSeat Extism host enables WASI. A relative clock subscription gives
    // this wasm32-unknown-unknown plugin a real, non-spinning delay.
    match unsafe { wasi::poll_oneoff(&subscription, event.as_mut_ptr(), 1) } {
        Ok(1) => Ok(()),
        _ => Err(()),
    }
}
