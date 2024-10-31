use std::sync::Arc;

use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, Mutex};

#[derive(Clone)]
pub struct MultiProducer<T> {
    receiver: Arc<Mutex<Receiver<T>>>,
    sender: mpsc::Sender<T>,
}

impl<T: Send + 'static> MultiProducer<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self { receiver: Arc::new(Mutex::new(receiver)), sender }
    }

    pub async fn recv(&mut self) -> Option<T> {
        self.receiver.lock().await.recv().await
    }

    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.sender.send(value).await
    }
}
/*
impl Clone for MultiProducer<T>
where T : Send + 'static
{
    fn clone(&self) -> Self {
        Self{
            receiver : self.receiver.clone(),
            sender : self.sender.clone()
        }
    }
}
 */
