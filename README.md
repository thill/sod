# SOD: Service-Oriented Design

Traits, utilities, and common implementations to facilitiate [service-oriented design](https://en.wikipedia.org/wiki/Service-orientation_design_principles) in Rust.
The traits and tools in the core library provide concrete guidelines to help make a service-oriented design successful.

## Core

The [sod core crate](./sod) provides all of the service traits and service-chaining utilities.

## Implementations

- [sod-actix-web](./sod-actix-web)
- [sod-bus](./sod-bus)
- [sod-crossbeam](./sod-crossbeam)
- [sod-log](./sod-log)
- [sod-mpsc](./sod-mpsc)
- [sod-tcp](./sod-tcp)
- [sod-tungstenite](./sod-tungstenite)
