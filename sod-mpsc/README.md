# sod-mpsc

[`sod::Service`](http://github.com/thill/sod) implementations to interact with `std::sync::mpsc` queues.

## Service Impls

- `MpscSender` sends to a `std::sync::mpsc::channel`.
- `MpscSyncSender` sends to a `std::sync::mpsc::sync_channel` and blocks if the channel is full.
- `MpscSyncTrySender` tries to send to a `std::sync::mpsc::sync_channel` and is able to be retried via `sod::RetryService` when the channel is full.
- `MpscReceiver` receives from a `std::sync::mpsc::channel` or `std::sync::mpsc::sync_channel`, blocking until an element is received.
- `MpscTryReceiver` tries to receive from a `std::sync::mpsc::channel` or `std::sync::mpsc::sync_channel`, and is able to be retried via `sod::Retryable` when the channel is empty.

## Example

```rust
use sod::Service;
use sod_mpsc::{MpscSender, MpscReceiver};
use std::sync::mpsc;

let (tx, rx) = mpsc::channel();
let pusher = MpscSender::new(tx);
let poller = MpscReceiver::new(rx);

pusher.process(1).unwrap();
pusher.process(2).unwrap();

assert_eq!(poller.process(()).unwrap(), 1);
assert_eq!(poller.process(()).unwrap(), 2);
```
