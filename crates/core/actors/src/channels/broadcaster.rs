use eyre::Result;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::SendError;
use tokio::sync::broadcast::Receiver;

#[derive(Clone)]
pub struct Broadcaster<T>
where
    T: Clone + Send + Sync + 'static,
{
    sender: broadcast::Sender<T>,
}

impl<T: Clone + Send + Sync + 'static> Broadcaster<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn send(&self, value: T) -> Result<usize, SendError<T>> {
        self.sender.send(value)
    }

    /*
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
     */

    pub fn subscribe(&self) -> Receiver<T> {
        self.sender.subscribe()
    }
}
