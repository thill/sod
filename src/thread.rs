use std::thread::{spawn, JoinHandle};

use crate::{MutService, Service};

/// Spawn a [`Service<()>`] in a new thread, calling [`Service::process`] repeatedly, until the given `error_handler` function returns `Err(_)`.
///
/// # Arguments
/// * `service` - the service to be called repeatedly
/// * `error_handler` - a function to handle errors, the result of which will determine if the thread should exit or keep running.
pub fn spawn_loop<S, F>(service: S, error_handler: F) -> JoinHandle<()>
where
    S: Service<()> + Send + 'static,
    F: Fn(S::Error) -> Result<(), S::Error> + Send + 'static,
{
    spawn(move || loop {
        if let Err(err) = service.process(()) {
            if let Err(_) = error_handler(err) {
                return;
            }
        }
    })
}

/// Spawn a [`MutService<()>`] in a new thread, calling [`Service::process`] repeatedly, until the given `error_handler` function returns `Err(_)`.
///
/// # Arguments
/// * `service` - the service to be called repeatedly
/// * `error_handler` - a function to handle errors, the result of which will determine if the thread should exit or keep running.
pub fn spawn_loop_mut<S, F>(mut service: S, error_handler: F) -> JoinHandle<()>
where
    S: MutService<()> + Send + 'static,
    F: Fn(S::Error) -> Result<(), S::Error> + Send + 'static,
{
    spawn(move || loop {
        if let Err(err) = service.process(()) {
            if let Err(_) = error_handler(err) {
                return;
            }
        }
    })
}
