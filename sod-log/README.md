# sod-log

[`sod::Service`](http://github.com/thill/sod) logging implementations via [`log`](https://crates.io/crates/log).

## Service Impls

- `LogDebugService` logs `Debug` input at a configured log level to `log::log`, returning the input as output.
- `LogDisplayService` logs `Display` input at a configured log level to `log::log`, returning the input as output.

## Use Case

These `Service` impls are most useful for logging an event as it passes through a service chain.

## Example

```rust
use sod::Service;
use sod_log::LogDisplayService;

let logging_service = LogDisplayService::info("my event: ");
logging_service.process("hello world!").unwrap();
```
