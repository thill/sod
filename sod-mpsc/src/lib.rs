//! [`sod::Service`] implementations to interact with [`std::sync::mpsc`] queues.
//!
//! ## Service Impls
//! * [`MpscSender`] sends to a [`std::sync::mpsc::channel`].
//! * [`MpscSyncSender`] sends to a [`std::sync::mpsc::sync_channel`] and blocks if the channel is full.
//! * [`MpscSyncTrySender`] tries to send to a [`std::sync::mpsc::sync_channel`] and is able to be retried via [`sod::RetryService`] when the channel is full.
//! * [`MpscReceiver`] receives from a [`std::sync::mpsc::channel`] or [`std::sync::mpsc::sync_channel`], blocking until an element is received.
//! * [`MpscTryReceiver`] tries to receive from a [`std::sync::mpsc::channel`] or [`std::sync::mpsc::sync_channel`], and is able to be retried via [`sod::RetryService`] when the channel is empty.
//!
//! ## Example
//! ```
//! use sod::Service;
//! use sod_mpsc::{MpscSender, MpscReceiver};
//! use std::sync::mpsc;
//!
//! let (tx, rx) = mpsc::channel();
//! let pusher = MpscSender::new(tx);
//! let poller = MpscReceiver::new(rx);
//!
//! pusher.process(1).unwrap();
//! pusher.process(2).unwrap();
//!
//! assert_eq!(poller.process(()).unwrap(), 1);
//! assert_eq!(poller.process(()).unwrap(), 2);
//! ```

use sod::{RetryError, Retryable, Service};
use std::sync::mpsc::{
    Receiver, RecvError, SendError, Sender, SyncSender, TryRecvError, TrySendError,
};

/// A non-blocking [`sod::Service`] that sends to an underlying [`std::sync::mpsc::Sender`].
#[derive(Clone)]
pub struct MpscSender<T> {
    tx: Sender<T>,
}
impl<T> MpscSender<T> {
    pub fn new(tx: Sender<T>) -> Self {
        Self { tx }
    }
}
impl<T> Service for MpscSender<T> {
    type Input = T;
    type Output = ();
    type Error = SendError<T>;
    fn process(&self, input: T) -> Result<Self::Output, Self::Error> {
        self.tx.send(input)
    }
}

/// A blocking [`sod::Service`] that sends to an underlying [`std::sync::mpsc::SyncSender`] using the `send` function.
#[derive(Clone)]
pub struct MpscSyncSender<T> {
    tx: SyncSender<T>,
}
impl<T> MpscSyncSender<T> {
    pub fn new(tx: SyncSender<T>) -> Self {
        Self { tx }
    }
}
impl<T> Service for MpscSyncSender<T> {
    type Input = T;
    type Output = ();
    type Error = SendError<T>;
    fn process(&self, input: T) -> Result<Self::Output, Self::Error> {
        self.tx.send(input)
    }
}

/// A non-blocking [`sod::Service`] that is [`sod::Retryable`] and sends to an underlying [`std::sync::mpsc::SyncSender`] using the `try_send` function.
#[derive(Clone)]
pub struct MpscSyncTrySender<T> {
    tx: SyncSender<T>,
}
impl<T> MpscSyncTrySender<T> {
    pub fn new(tx: SyncSender<T>) -> Self {
        Self { tx }
    }
}
impl<T> Service for MpscSyncTrySender<T> {
    type Input = T;
    type Output = ();
    type Error = TrySendError<T>;
    fn process(&self, input: T) -> Result<Self::Output, Self::Error> {
        self.tx.try_send(input)
    }
}
impl<T> Retryable<T, TrySendError<T>> for MpscSyncTrySender<T> {
    fn parse_retry(&self, err: TrySendError<T>) -> Result<T, RetryError<TrySendError<T>>> {
        match err {
            TrySendError::Full(input) => Ok(input),
            err => Err(RetryError::ServiceError(err)),
        }
    }
}

/// A blocking [`sod::Service`] that receives from an underlying [`std::sync::mpsc::Receiver`], blocking per the rules of `Receiver::recv`
pub struct MpscReceiver<T> {
    rx: Receiver<T>,
}
impl<T> MpscReceiver<T> {
    pub fn new(rx: Receiver<T>) -> Self {
        Self { rx }
    }
}
impl<T> Service for MpscReceiver<T> {
    type Input = ();
    type Output = T;
    type Error = RecvError;
    fn process(&self, _: ()) -> Result<Self::Output, Self::Error> {
        self.rx.recv()
    }
}

/// A non-blocking [`sod::Service`] that is [`sod::Retryable`] and receives from an underlying [`std::sync::mpsc::Receiver`], blocking per the rules of `Receiver::recv`
pub struct MpscTryReceiver<T> {
    rx: Receiver<T>,
}
impl<T> MpscTryReceiver<T> {
    pub fn new(rx: Receiver<T>) -> Self {
        Self { rx }
    }
}
impl<T> Service for MpscTryReceiver<T> {
    type Input = ();
    type Output = T;
    type Error = TryRecvError;
    fn process(&self, _: ()) -> Result<Self::Output, Self::Error> {
        self.rx.try_recv()
    }
}
impl<T> Retryable<(), TryRecvError> for MpscTryReceiver<T> {
    fn parse_retry(&self, err: TryRecvError) -> Result<(), RetryError<TryRecvError>> {
        match err {
            TryRecvError::Empty => Ok(()),
            err => Err(RetryError::ServiceError(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn channel() {
        let (tx, rx) = mpsc::channel();
        let pusher = MpscSender::new(tx);
        let poller = MpscReceiver::new(rx);

        pusher.process(1).unwrap();
        pusher.process(2).unwrap();
        pusher.process(3).unwrap();

        assert_eq!(poller.process(()).unwrap(), 1);
        assert_eq!(poller.process(()).unwrap(), 2);
        assert_eq!(poller.process(()).unwrap(), 3);
    }

    #[test]
    fn sync_channel() {
        let (tx, rx) = mpsc::sync_channel(5);
        let pusher = MpscSyncSender::new(tx);
        let poller = MpscReceiver::new(rx);

        pusher.process(1).unwrap();
        pusher.process(2).unwrap();
        pusher.process(3).unwrap();

        assert_eq!(poller.process(()), Ok(1));
        assert_eq!(poller.process(()), Ok(2));
        assert_eq!(poller.process(()), Ok(3));
    }

    #[test]
    fn try_sync_channel() {
        let (tx, rx) = mpsc::sync_channel(5);
        let pusher = MpscSyncTrySender::new(tx);
        let poller = MpscTryReceiver::new(rx);

        pusher.process(1).unwrap();
        pusher.process(2).unwrap();
        pusher.process(3).unwrap();

        assert_eq!(poller.process(()), Ok(1));
        assert_eq!(poller.process(()), Ok(2));
        assert_eq!(poller.process(()), Ok(3));
        assert_eq!(poller.process(()), Err(TryRecvError::Empty));
    }
}
