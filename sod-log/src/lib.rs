//! [`sod::Service`] logging implementations via [`log`](https://crates.io/crates/log).
//!
//! ## Service Impls
//! * [`LogDebugService`] logs [`Debug`] input at a configured log level to [`log::log`], returning the input as output.
//! * [`LogDisplayService`] logs [`Display`] input at a configured log level to [`log::log`], returning the input as output.
//!
//! ## Use Case
//! These [`Service`] impls are most useful for logging an event as it passes through a service chain.
//!
//! ## Example
//! ```
//! use sod::Service;
//! use sod_log::LogDisplayService;
//!
//! let logging_service = LogDisplayService::info("my event: ");
//! logging_service.process("hello world!").unwrap();
//! ```

use std::{
    borrow::Cow,
    fmt::{Debug, Display},
    marker::PhantomData,
};

use log::Level;
use sod::Service;

/// A [`sod::Service`] that logs [`Debug`] input at a configured log level to [`log::log`], returning the input as output.
pub struct LogDebugService<'a, T> {
    level: Level,
    prefix: Cow<'a, str>,
    _phantom: PhantomData<fn(T)>,
}
impl<'a, T> LogDebugService<'a, T> {
    /// Log input at the given log level
    /// # Arguments
    /// * `level` - The log level
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn new<S: Into<Cow<'a, str>>>(level: Level, prefix: S) -> Self {
        Self {
            level,
            prefix: prefix.into(),
            _phantom: PhantomData,
        }
    }
    /// Log as [`Level::Debug`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn debug<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Debug, prefix)
    }
    /// Log as [`Level::Error`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn error<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Error, prefix)
    }
    /// Log as [`Level::Info`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn info<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Info, prefix)
    }
    /// Log as [`Level::Trace`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn trace<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Trace, prefix)
    }
    /// Log as [`Level::Warn`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn warn<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Warn, prefix)
    }
}
impl<'a, T: Debug> Service for LogDebugService<'a, T> {
    type Input = T;
    type Output = T;
    type Error = ();
    fn process(&self, input: T) -> Result<Self::Output, Self::Error> {
        log::log!(self.level, "{}{:?}", self.prefix, input);
        Ok(input)
    }
}

/// A [`sod::Service`] that logs optional [`Debug`] input when it is `Some(input)` at a configured log level to [`log::log`], returning the input as output.
///
/// This service is useful for logging an event as it passed through a service chain, while ignoring non-blocking service chains that may continuously process `None` in a tight loop.
pub struct LogOptionalDebugService<'a, T> {
    level: Level,
    prefix: Cow<'a, str>,
    _phantom: PhantomData<fn(T)>,
}
impl<'a, T> LogOptionalDebugService<'a, T> {
    /// Log input at the given log level
    /// # Arguments
    /// * `level` - The log level
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn new<S: Into<Cow<'a, str>>>(level: Level, prefix: S) -> Self {
        Self {
            level,
            prefix: prefix.into(),
            _phantom: PhantomData,
        }
    }
    /// Log as [`Level::Debug`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn debug<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Debug, prefix)
    }
    /// Log as [`Level::Error`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn error<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Error, prefix)
    }
    /// Log as [`Level::Info`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn info<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Info, prefix)
    }
    /// Log as [`Level::Trace`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn trace<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Trace, prefix)
    }
    /// Log as [`Level::Warn`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn warn<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Warn, prefix)
    }
}
impl<'a, T: Debug> Service for LogOptionalDebugService<'a, T> {
    type Input = Option<T>;
    type Output = Option<T>;
    type Error = ();
    fn process(&self, input: Option<T>) -> Result<Self::Output, Self::Error> {
        if let Some(input) = &input {
            log::log!(self.level, "{}{:?}", self.prefix, input);
        }
        Ok(input)
    }
}

/// A [`sod::Service`] that logs [`Display`] input at a configured log level to [`log::log`], returning the input as output.
///
/// This service is useful for logging an event as it passed through a service chain.
pub struct LogDisplayService<'a, T> {
    level: Level,
    prefix: Cow<'a, str>,
    _phantom: PhantomData<fn(T)>,
}
impl<'a, T> LogDisplayService<'a, T> {
    /// Log input at the given log level
    /// # Arguments
    /// * `level` - The log level
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn new<S: Into<Cow<'a, str>>>(level: Level, prefix: S) -> Self {
        Self {
            level,
            prefix: prefix.into(),
            _phantom: PhantomData,
        }
    }
    /// Log as [`Level::Debug`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn debug<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Debug, prefix)
    }
    /// Log as [`Level::Error`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn error<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Error, prefix)
    }
    /// Log as [`Level::Info`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn info<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Info, prefix)
    }
    /// Log as [`Level::Trace`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn trace<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Trace, prefix)
    }
    /// Log as [`Level::Warn`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn warn<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Warn, prefix)
    }
}
impl<'a, T: Display> Service for LogDisplayService<'a, T> {
    type Input = T;
    type Output = T;
    type Error = ();
    fn process(&self, input: T) -> Result<Self::Output, Self::Error> {
        log::log!(self.level, "{}{}", self.prefix, input);
        Ok(input)
    }
}

/// A [`sod::Service`] that logs optional [`Display`] input when it is `Some(input)` at a configured log level to [`log::log`], returning the input as output.
///
/// This service is useful for logging an event as it passed through a service chain, while ignoring non-blocking service chains that may continuously process `None` in a tight loop.
pub struct LogOptionalDisplayService<'a, T> {
    level: Level,
    prefix: Cow<'a, str>,
    _phantom: PhantomData<fn(T)>,
}
impl<'a, T> LogOptionalDisplayService<'a, T> {
    /// Log input at the given log level
    /// # Arguments
    /// * `level` - The log level
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn new<S: Into<Cow<'a, str>>>(level: Level, prefix: S) -> Self {
        Self {
            level,
            prefix: prefix.into(),
            _phantom: PhantomData,
        }
    }
    /// Log as [`Level::Debug`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn debug<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Debug, prefix)
    }
    /// Log as [`Level::Error`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn error<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Error, prefix)
    }
    /// Log as [`Level::Info`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn info<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Info, prefix)
    }
    /// Log as [`Level::Trace`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn trace<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Trace, prefix)
    }
    /// Log as [`Level::Warn`]
    /// # Arguments
    /// * `prefix` - A prefix to prepend to the beginning of the log statment
    pub fn warn<S: Into<Cow<'a, str>>>(prefix: S) -> Self {
        Self::new(Level::Warn, prefix)
    }
}
impl<'a, T: Display> Service for LogOptionalDisplayService<'a, T> {
    type Input = Option<T>;
    type Output = Option<T>;
    type Error = ();
    fn process(&self, input: Option<T>) -> Result<Self::Output, Self::Error> {
        if let Some(input) = &input {
            log::log!(self.level, "{}{}", self.prefix, input);
        }
        Ok(input)
    }
}
