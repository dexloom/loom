use crate::Swap;
use alloy_primitives::{Bytes, U256};
use eyre::Result;
use std::ops::Deref;
use std::sync::Arc;

pub trait SwapEncoder {
    fn encode(&self, swap: Swap, bribe: Option<U256>) -> Result<Bytes>
    where
        Self: Sized;
}

#[derive(Clone)]
pub struct SwapEncoderWrapper {
    pub inner: Arc<dyn SwapEncoder>,
}

impl SwapEncoderWrapper {
    pub fn new(encoder: Arc<dyn SwapEncoder>) -> Self {
        SwapEncoderWrapper { inner: encoder }
    }
}

impl<T: 'static + SwapEncoder + Clone> From<T> for SwapEncoderWrapper {
    fn from(pool: T) -> Self {
        Self { inner: Arc::new(pool) }
    }
}

impl Deref for SwapEncoderWrapper {
    type Target = dyn SwapEncoder;
    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}
