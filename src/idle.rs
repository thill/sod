//! Idle functions for use by [`crate::RetryService`] and [`crate::PollService`].
//!
//! Utility functions for common idle strategies.
//! These idle strategies will all first check the static [`KEEP_RUNNING`] boolean, and will return `Err(RetryError::Interrupted)` when `KEEP_RUNNING` returns false.
//!
//! For an idle strategy with a good balance between performance and CPU-spinning, see [`backoff`].

use crate::RetryError;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

/// Defaults to true, can be set to false to terminate all idle strategies.
///
/// Here is an example to use the `ctrlc` crate to set this [`AtomicBool`] to false to gracefully terminate any idle loops when a `SIGINT` is received by the process:
/// ```
/// use std::sync::atomic::Ordering;
///
/// ctrlc::set_handler(move || {
///    sod::idle::KEEP_RUNNING.store(false, Ordering::SeqCst);
/// }).expect("Error setting Ctrl-C handler");
/// ```
pub static KEEP_RUNNING: AtomicBool = AtomicBool::new(true);

/// First busy spin for 10 cycles, then yield for 10 cycles, then park for 1us, increasing by powers of two each attempt, maxing out at 1024us (1.024ms).
pub fn backoff<E>(attempts: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    if attempts < 10 {
    } else if attempts < 20 {
        thread::yield_now();
    } else if attempts < 30 {
        let micros = 1 << (attempts - 20);
        thread::park_timeout(Duration::from_micros(micros));
    } else {
        thread::park_timeout(Duration::from_micros(1024));
    }
    Ok(())
}

/// No-Op
pub fn spin<E>(_: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    Ok(())
}

/// Calls [`std::thread::yield_now()`]
pub fn yielding<E>(_: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    thread::yield_now();
    Ok(())
}

/// Calls [`std::thread::park_timeout`] with the given timeout
pub fn park<E>(_: usize, timeout: Duration) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    thread::park_timeout(timeout);
    Ok(())
}

/// Calls [`std::thread::park_timeout`] with a 1us timeout
pub fn park_one_micro<E>(attempts: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    park(attempts, Duration::from_micros(1))
}

/// Calls [`std::thread::park_timeout`] with a 1ms timeout
pub fn park_one_milli<E>(attempts: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    park(attempts, Duration::from_millis(1))
}

/// Calls [`std::thread::park_timeout`] with a 1s timeout
pub fn park_one_sec<E>(attempts: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    park(attempts, Duration::from_secs(1))
}

/// Calls [`std::thread::sleep`] with the given duration
pub fn sleep<E>(_: usize, duration: Duration) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    thread::sleep(duration);
    Ok(())
}

/// Calls [`std::thread::sleep`] with a 1us duration
pub fn sleep_one_micro<E>(_: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    thread::sleep(Duration::from_micros(1));
    Ok(())
}

/// Calls [`std::thread::sleep`] with a 1ms duration
pub fn sleep_one_milli<E>(_: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    thread::sleep(Duration::from_millis(1));
    Ok(())
}

/// Calls [`std::thread::sleep`] with a 1s duration
pub fn sleep_one_sec<E>(_: usize) -> Result<(), RetryError<E>> {
    check_keep_running()?;
    thread::sleep(Duration::from_secs(1));
    Ok(())
}

fn check_keep_running<E>() -> Result<(), RetryError<E>> {
    if KEEP_RUNNING.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err(RetryError::Interrupted)
    }
}
