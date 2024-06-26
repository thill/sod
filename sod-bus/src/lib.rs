//! [`sod::MutService`] implementations to interact with the broadcast [`bus::Bus`].
//!
//! ## Service Impls
//! * [`BusBroadcaster`] broadcasts to a [`bus::Bus`] and blocks until the operation is successful.
//! * [`BusTryBroadcaster`] tries to broadcast to a [`bus::Bus`] and is able to be retried via [`sod::RetryService`] when the bus buffer is full.
//! * [`BusReceiver`] receives from a [`bus::BusReader`], blocking until an element is received.
//! * [`BusTryReceiver`] tries to receive from a [`bus::BusReader`] and is able to be retried via [`sod::RetryService`] when the bus is empty.
//!
//! ## Example
//! ```
//! use sod::MutService;
//! use sod_bus::{BusBroadcaster, BusReceiver};
//!
//! let mut broadcaster = BusBroadcaster::with_len(1024);
//! let mut receiver1 = broadcaster.create_receiver();
//! let mut receiver2 = broadcaster.create_receiver();
//!
//! broadcaster.process(1).unwrap();
//! broadcaster.process(2).unwrap();
//! broadcaster.process(3).unwrap();
//!
//! assert_eq!(receiver1.process(()).unwrap(), 1);
//! assert_eq!(receiver1.process(()).unwrap(), 2);
//! assert_eq!(receiver1.process(()).unwrap(), 3);
//!
//! assert_eq!(receiver2.process(()).unwrap(), 1);
//! assert_eq!(receiver2.process(()).unwrap(), 2);
//! assert_eq!(receiver2.process(()).unwrap(), 3);
//! ```

use bus::{Bus, BusReader};
use sod::{MutService, RetryError, Retryable};
use std::{
    convert::Infallible,
    sync::mpsc::{RecvError, TryRecvError},
};

/// A blocking [`sod::MutService`] that broadcasts to an underlying [`bus::Bus`].
pub struct BusBroadcaster<T> {
    bus: Bus<T>,
}
impl<T> BusBroadcaster<T> {
    /// encapsulate the given [`Bus`]
    pub fn new(bus: Bus<T>) -> Self {
        Self { bus }
    }
    /// create a new [`Bus`] with the given length
    pub fn with_len(len: usize) -> Self {
        Self { bus: Bus::new(len) }
    }
    /// get a mutable reference to the underlying bus, which may be used to add readers
    pub fn bus<'a>(&'a mut self) -> &'a mut Bus<T> {
        &mut self.bus
    }
    /// create a [`BusReceiver`] service from the underlying [`Bus`].
    pub fn create_receiver(&mut self) -> BusReceiver<T> {
        BusReceiver::new(self.bus.add_rx())
    }
    /// create a [`BusTryReceiver`] service from the underlying [`Bus`].
    pub fn create_try_receiver(&mut self) -> BusTryReceiver<T> {
        BusTryReceiver::new(self.bus.add_rx())
    }
}
impl<T> MutService for BusBroadcaster<T> {
    type Input = T;
    type Output = ();
    type Error = Infallible;
    fn process(&mut self, input: T) -> Result<Self::Output, Self::Error> {
        Ok(self.bus.broadcast(input))
    }
}

/// A non-blocking [`sod::MutService`] that is [`Retryable`] and attempts to broadcast to an underlying [`bus::Bus`].
pub struct BusTryBroadcaster<T> {
    bus: Bus<T>,
}
impl<T> BusTryBroadcaster<T> {
    /// encapsulate the given [`Bus`]
    pub fn new(bus: Bus<T>) -> Self {
        Self { bus }
    }
    /// create a new [`Bus`] with the given length
    pub fn with_len(len: usize) -> Self {
        Self { bus: Bus::new(len) }
    }
    /// get a mutable reference to the underlying bus, which may be used to add readers
    pub fn bus<'a>(&'a mut self) -> &'a mut Bus<T> {
        &mut self.bus
    }
    /// create a [`BusReceiver`] service from the underlying [`Bus`].
    pub fn create_receiver(&mut self) -> BusReceiver<T> {
        BusReceiver::new(self.bus.add_rx())
    }
    /// create a [`BusTryReceiver`] service from the underlying [`Bus`].
    pub fn create_try_receiver(&mut self) -> BusTryReceiver<T> {
        BusTryReceiver::new(self.bus.add_rx())
    }
}
impl<T> MutService for BusTryBroadcaster<T> {
    type Input = T;
    type Output = ();
    type Error = T;
    fn process(&mut self, input: T) -> Result<Self::Output, Self::Error> {
        self.bus.try_broadcast(input)
    }
}
impl<T> Retryable<T, T> for BusTryBroadcaster<T> {
    fn parse_retry(&self, err: T) -> Result<T, RetryError<T>> {
        Ok(err)
    }
}

/// A blocking [`sod::MutService`] that receives from an underlying [`bus::BusReader`]
pub struct BusReceiver<T> {
    reader: BusReader<T>,
}
impl<T> BusReceiver<T> {
    pub fn new(reader: BusReader<T>) -> Self {
        Self { reader }
    }
}
impl<T: Clone + Sync> MutService for BusReceiver<T> {
    type Input = ();
    type Output = T;
    type Error = RecvError;
    fn process(&mut self, _: ()) -> Result<Self::Output, Self::Error> {
        self.reader.recv()
    }
}

/// A non-blocking [`sod::MutService`] that is [`sod::Retryable`] and receives from an underlying [`bus::BusReader`]
pub struct BusTryReceiver<T> {
    reader: BusReader<T>,
}
impl<T> BusTryReceiver<T> {
    pub fn new(reader: BusReader<T>) -> Self {
        Self { reader }
    }
}
impl<T: Clone + Sync> MutService for BusTryReceiver<T> {
    type Input = ();
    type Output = T;
    type Error = TryRecvError;
    fn process(&mut self, _: ()) -> Result<Self::Output, Self::Error> {
        self.reader.try_recv()
    }
}
impl<T: Clone + Sync> Retryable<(), TryRecvError> for BusTryReceiver<T> {
    fn parse_retry(&self, err: TryRecvError) -> Result<(), RetryError<TryRecvError>> {
        match err {
            TryRecvError::Disconnected => Err(RetryError::ServiceError(TryRecvError::Disconnected)),
            TryRecvError::Empty => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking() {
        let mut broadcaster = BusBroadcaster::new(Bus::new(1024));
        let mut reader1 = broadcaster.create_receiver();
        let mut reader2 = broadcaster.create_receiver();
        broadcaster.process(1).unwrap();
        broadcaster.process(2).unwrap();
        broadcaster.process(3).unwrap();

        assert_eq!(reader1.process(()).unwrap(), 1);
        assert_eq!(reader1.process(()).unwrap(), 2);
        assert_eq!(reader1.process(()).unwrap(), 3);

        assert_eq!(reader2.process(()).unwrap(), 1);
        assert_eq!(reader2.process(()).unwrap(), 2);
        assert_eq!(reader2.process(()).unwrap(), 3);
    }

    #[test]
    fn non_blocking() {
        let mut broadcaster = BusTryBroadcaster::new(Bus::new(1024));
        let mut reader1 = broadcaster.create_try_receiver();
        let mut reader2 = broadcaster.create_try_receiver();

        broadcaster.process(1).unwrap();
        broadcaster.process(2).unwrap();
        broadcaster.process(3).unwrap();

        assert_eq!(reader1.process(()).unwrap(), 1);
        assert_eq!(reader1.process(()).unwrap(), 2);
        assert_eq!(reader1.process(()).unwrap(), 3);

        assert_eq!(reader2.process(()).unwrap(), 1);
        assert_eq!(reader2.process(()).unwrap(), 2);
        assert_eq!(reader2.process(()).unwrap(), 3);
        assert_eq!(reader2.process(()), Err(TryRecvError::Empty));
    }
}
