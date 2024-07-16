use std::sync::Arc;

use eyre::Result;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};

//#[derive(Clone)]
pub struct SharedState<T> {
    inner: Arc<RwLock<T>>,
}

impl<T> SharedState<T> {
    pub fn new(shared_data: T) -> SharedState<T> {
        SharedState { inner: Arc::new(RwLock::new(shared_data)) }
    }

    pub async fn read(&self) -> RwLockReadGuard<T> {
        self.inner.read().await
    }

    pub fn try_read(&self) -> Result<RwLockReadGuard<T>, TryLockError> {
        self.inner.try_read()
    }

    pub async fn write(&self) -> RwLockWriteGuard<T> {
        self.inner.write().await
    }

    pub fn try_write(&self) -> Result<RwLockWriteGuard<T>, TryLockError> {
        self.inner.try_write()
    }

    pub fn inner(&self) -> Arc<RwLock<T>> {
        self.inner.clone()
    }

    pub async fn update(&self, inner: T) {
        let mut guard = self.inner.write().await;
        *guard = inner
    }
}

impl<T> Clone for SharedState<T> {
    fn clone(&self) -> Self {
        SharedState { inner: self.inner().clone() }
    }
}
