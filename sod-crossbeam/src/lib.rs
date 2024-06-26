//! [`sod::Service`] implementations to interact with [`crossbeam`] queues.
//!
//! ## Service Impls
//! * [`ArrayQueue`] can be pushed with [`ArrayQueuePusher`] or [`ArrayQueueForcePusher`] and popped with [`ArrayQueuePopper`]
//! * [`SegQueue`] can be pushed with [`SegQueuePusher`] or [`SegQueuePopper`]
//!
//! ## Async
//!
//! Any of the services can be represented as an `AsyncService` using [`sod::IntoAsyncService::into_async].
//!
//! ```
//! use crossbeam::queue::ArrayQueue;
//! use sod::{PollService, Service};
//! use sod_crossbeam::ArrayQueuePopper;
//! use std::{sync::Arc, time::Duration};
//!
//! let q = Arc::new(ArrayQueue::<i32>::new(128));
//! let async_popper = ArrayQueuePopper::new(Arc::clone(&q)).into_mut();
//! ```
//!
//! ## Blocking
//!
//! [`sod::PollService`] may encapsulate a [`ArrayQueuePopper`] or [`SegQueuePopper`] to provide a backoff mechanism to avoid busy-spinning the CPU in a poll loop.
//!
//! ```no_run
//! use crossbeam::queue::ArrayQueue;
//! use sod::{idle::backoff, PollService, Service};
//! use sod_crossbeam::ArrayQueuePopper;
//! use std::{sync::Arc, time::Duration};
//!
//! let q = Arc::new(ArrayQueue::<i32>::new(128));
//! let popper = PollService::new(ArrayQueuePopper::new(Arc::clone(&q)), backoff);
//!
//! loop {
//!     println!("received: {}", popper.process(()).unwrap());
//! }
//! ```
//!
//! [`sod::RetryService`] may encapsulate a [`ArrayQueuePusher`] to block and continuously retry pushing an element to an [`ArrayQueue`] until it succeeds.
//!
//! ```
//! use crossbeam::queue::ArrayQueue;
//! use sod::{RetryService, Service, idle::yielding};
//! use sod_crossbeam::ArrayQueuePusher;
//! use std::sync::Arc;
//!
//! let q = Arc::new(ArrayQueue::new(128));
//! let pusher = RetryService::new(ArrayQueuePusher::new(Arc::clone(&q)), yielding);
//!
//! pusher.process(123).unwrap();
//! pusher.process(456).unwrap();
//! ```

use std::{convert::Infallible, sync::Arc};

use crossbeam::queue::{ArrayQueue, SegQueue};
use sod::{Retryable, Service};

/// A [`sod::Service`] that is [`sod::Retryable`] and pushes input to an underlying [`crossbeam::queue::ArrayQueue`], returning the element as an error when the queue is full.
pub struct ArrayQueuePusher<T> {
    q: Arc<ArrayQueue<T>>,
}
impl<T> ArrayQueuePusher<T> {
    pub fn new(q: Arc<ArrayQueue<T>>) -> Self {
        Self { q }
    }
}
impl<T> Service for ArrayQueuePusher<T> {
    type Input = T;
    type Output = ();
    type Error = T;
    fn process(&self, input: T) -> Result<Self::Output, Self::Error> {
        self.q.push(input)
    }
}
impl<T> Retryable<T, T> for ArrayQueuePusher<T> {
    fn parse_retry(&self, err: T) -> Result<T, sod::RetryError<T>> {
        Ok(err)
    }
}

/// A [`sod::Service`] that force pushes input to an underlying [`crossbeam::queue::ArrayQueue`].
pub struct ArrayQueueForcePusher<T> {
    q: Arc<ArrayQueue<T>>,
}
impl<T> ArrayQueueForcePusher<T> {
    pub fn new(q: Arc<ArrayQueue<T>>) -> Self {
        Self { q }
    }
}
impl<T> Service for ArrayQueueForcePusher<T> {
    type Input = T;
    type Output = Option<T>;
    type Error = Infallible;
    fn process(&self, input: T) -> Result<Self::Output, Self::Error> {
        Ok(self.q.force_push(input))
    }
}

/// A [`sod::Service`] that pops an element from an underlying [`crossbeam::queue::ArrayQueue`].
pub struct ArrayQueuePopper<T> {
    q: Arc<ArrayQueue<T>>,
}
impl<T> ArrayQueuePopper<T> {
    pub fn new(q: Arc<ArrayQueue<T>>) -> Self {
        Self { q }
    }
}
impl<T> Service for ArrayQueuePopper<T> {
    type Input = ();
    type Output = Option<T>;
    type Error = Infallible;
    fn process(&self, _: ()) -> Result<Self::Output, Self::Error> {
        Ok(self.q.pop())
    }
}

/// A [`sod::Service`] that pushes input to an underlying [`crossbeam::queue::SegQueue`].
pub struct SegQueuePusher<T> {
    q: Arc<SegQueue<T>>,
}
impl<T> SegQueuePusher<T> {
    pub fn new(q: Arc<SegQueue<T>>) -> Self {
        Self { q }
    }
}
impl<T> Service for SegQueuePusher<T> {
    type Input = T;
    type Output = ();
    type Error = Infallible;
    fn process(&self, input: T) -> Result<Self::Output, Self::Error> {
        Ok(self.q.push(input))
    }
}

/// A [`sod::Service`] that pops an element from an underlying [`crossbeam::queue::SegQueue`].
pub struct SegQueuePopper<T> {
    q: Arc<SegQueue<T>>,
}
impl<T> SegQueuePopper<T> {
    pub fn new(q: Arc<SegQueue<T>>) -> Self {
        Self { q }
    }
}
impl<T> Service for SegQueuePopper<T> {
    type Input = ();
    type Output = Option<T>;
    type Error = Infallible;
    fn process(&self, _: ()) -> Result<Self::Output, Self::Error> {
        Ok(self.q.pop())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sod::{
        idle::{park_one_micro, yielding},
        PollService, RetryService,
    };
    use std::{sync::Arc, thread::spawn, time::Duration};

    #[test]
    fn array_queue() {
        let q = Arc::new(ArrayQueue::new(128));
        let pusher = ArrayQueuePusher::new(Arc::clone(&q));
        let poller = ArrayQueuePopper::new(q);

        pusher.process(1).unwrap();
        pusher.process(2).unwrap();
        pusher.process(3).unwrap();

        assert_eq!(poller.process(()).unwrap(), Some(1));
        assert_eq!(poller.process(()).unwrap(), Some(2));
        assert_eq!(poller.process(()).unwrap(), Some(3));
        assert_eq!(poller.process(()).unwrap(), None);
    }

    #[test]
    fn seg_queue() {
        let q = Arc::new(SegQueue::new());
        let pusher = SegQueuePusher::new(Arc::clone(&q));
        let poller = SegQueuePopper::new(q);

        pusher.process(1).unwrap();
        pusher.process(2).unwrap();
        pusher.process(3).unwrap();

        assert_eq!(poller.process(()).unwrap(), Some(1));
        assert_eq!(poller.process(()).unwrap(), Some(2));
        assert_eq!(poller.process(()).unwrap(), Some(3));
        assert_eq!(poller.process(()).unwrap(), None);
    }

    #[test]
    fn push_with_retry() {
        let q = Arc::new(ArrayQueue::new(128));
        let pusher = RetryService::new(ArrayQueuePusher::new(Arc::clone(&q)), park_one_micro);
        let poller = PollService::new(ArrayQueuePopper::new(q), yielding);

        // spin up new thread and quickly write 1024 entries to a queue with 128 capacity
        let j = spawn(move || {
            for i in 0..1024 {
                pusher.process(i).unwrap();
            }
        });

        // verify with slow consumer, forcing the pusher to block
        for i in 0..1024 {
            assert_eq!(poller.process(()).unwrap(), i);
            std::thread::sleep(Duration::from_millis(1));
        }

        j.join().unwrap();
    }
}
