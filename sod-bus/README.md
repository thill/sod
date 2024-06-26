# sod-bus

[`sod::MutService`](http://github.com/thill/sod) implementations to interact with [`bus::Bus`](https://crates.io/crates/bus).

## Service Impls

- `BusBroadcaster` broadcasts to a `bus::Bus` and blocks until the operation is successful.
- `BusTryBroadcaster` tries to broadcast to a `bus::Bus` and is able to be retried via `sod::RetryService` when the bus buffer is full.
- `BusReceiver` receives from a `bus::BusReader`, blocking until an element is received.
- `BusTryReceiver` tries to receive from a `bus::BusReader` and is able to be retried via `sod::RetryService` when the bus is empty.

## Example

```rust
use sod::MutService;
use sod_bus::{BusBroadcaster, BusReceiver};

let mut broadcaster = BusBroadcaster::with_len(1024);
let mut receiver1 = broadcaster.create_receiver();
let mut receiver2 = broadcaster.create_receiver();

broadcaster.process(1).unwrap();
broadcaster.process(2).unwrap();
broadcaster.process(3).unwrap();

assert_eq!(receiver1.process(()).unwrap(), 1);
assert_eq!(receiver1.process(()).unwrap(), 2);
assert_eq!(receiver1.process(()).unwrap(), 3);

assert_eq!(receiver2.process(()).unwrap(), 1);
assert_eq!(receiver2.process(()).unwrap(), 2);
assert_eq!(receiver2.process(()).unwrap(), 3);
```
