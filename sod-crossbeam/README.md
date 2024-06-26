# sod-crossbeam

[`sod::Service`](http://github.com/thill/sod) implementations to interact with `crossbeam` queues.

## Service Impls

- `ArrayQueuePusher` pushes to a `crossbeam::queue::ArrayQueue`
- `ArrayQueueForcePusher` force pushes to a `crossbeam::queue::ArrayQueue`
- `ArrayQueuePopper` pops from a `crossbeam::queue::ArrayQueue`
- `SegQueuePusher` pushes to a `crossbeam::queue::SegQueue`
- `SegQueuePopper` pops from a `crossbeam::queue::SegQueue`

## Async

Any of the services can be represented as an `AsyncService` using `self.into_async()`.

```
use crossbeam::queue::ArrayQueue;
use sod::{PollService, Service};
use sod_crossbeam::ArrayQueuePopper;
use std::{sync::Arc, time::Duration};

let q = Arc::new(ArrayQueue::<i32>::new(128));
let async_popper = ArrayQueuePopper::new(Arc::clone(&q)).into_mut();
```

## Blocking

`sod::PollService` may encapsulate a `ArrayQueuePopper` or `SegQueuePopper` to provide a backoff mechanism to avoid busy-spinning the CPU in a poll loop.

```rust
use crossbeam::queue::ArrayQueue;
use sod::{idle::backoff, PollService, Service};
use sod_crossbeam::ArrayQueuePopper;
use std::{sync::Arc, time::Duration};

let q = Arc::new(ArrayQueue::<i32>::new(128));
let popper = PollService::new(ArrayQueuePopper::new(Arc::clone(&q)), backoff);

loop {
    println!("received: {}", popper.process(()).unwrap());
}
```

`sod::RetryService` may encapsulate a `ArrayQueuePusher` to block and continuously retry pushing an element to an `ArrayQueue` until it succeeds.

```rust
use crossbeam::queue::ArrayQueue;
use sod::{RetryService, Service, idle::yielding};
use sod_crossbeam::ArrayQueuePusher;
use std::sync::Arc;

let q = Arc::new(ArrayQueue::new(128));
let pusher = RetryService::new(ArrayQueuePusher::new(Arc::clone(&q)), yielding);

pusher.process(123).unwrap();
pusher.process(456).unwrap();
```
