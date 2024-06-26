use std::{future::Future, pin::Pin};

pub struct SettableFuture<T: Unpin> {
    result: Option<T>,
}
impl<T: Unpin> SettableFuture<T> {
    pub fn new() -> Self {
        Self { result: None }
    }
    pub fn set(self, value: T) -> Self {
        Self {
            result: Some(value),
        }
    }
}
impl<T: Unpin> Future for SettableFuture<T> {
    type Output = T;
    fn poll(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let result = std::mem::take(&mut self.as_mut().result);
        match result {
            Some(result) => std::task::Poll::Ready(result),
            None => std::task::Poll::Pending,
        }
    }
}
