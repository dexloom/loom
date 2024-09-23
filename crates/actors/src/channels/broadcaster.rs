use std::sync::Arc;

use eyre::{eyre, Result};
use log::error;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::SendError;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Broadcaster<T>
where
    T: Clone + Send + Sync + 'static,
{
    sender: Arc<RwLock<broadcast::Sender<T>>>,
}

impl<T: Clone + Send + Sync + 'static> Broadcaster<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender: Arc::new(RwLock::new(sender)) }
    }

    pub async fn send(&self, value: T) -> Result<usize, SendError<T>> {
        let sender = self.sender.write().await;
        sender.send(value)
    }

    pub fn try_send(&self, value: T) -> Result<usize> {
        //let sender = self.sender.write().await;
        match self.sender.try_write() {
            Ok(guard) => match guard.send(value) {
                Ok(size) => Ok(size),
                Err(_) => Err(eyre!("ERROR_SEND")),
            },
            Err(e) => {
                error!("self.sender.try_write {}", e);
                Err(eyre!("ERROR_WRITE_LOCK"))
            }
        }
    }

    pub async fn subscribe(&self) -> Receiver<T> {
        let sender = self.sender.write().await;
        sender.subscribe()
    }

    pub fn subscribe_sync(&self) -> Result<Receiver<T>> {
        let sender = self.sender.try_write()?;
        Ok(sender.subscribe())
    }
}
