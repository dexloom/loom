use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;

use eyre::Result;
use tokio::sync::RwLock;
use tracing::error;

use crate::FetchState::Fetching;

#[derive(Debug, Clone)]
pub enum FetchState<T: Clone> {
    Fetching(Arc<RwLock<Option<T>>>),
    Ready(T),
}

#[derive(Debug, Clone, Default)]
pub struct DataFetcher<K, V>
where
    K: Clone + Default + Eq + PartialEq + Hash + Send + Sync + 'static,
    V: Clone + Default + Send + Sync + 'static,
{
    data: HashMap<K, FetchState<V>>,
}

impl<K, V> DataFetcher<K, V>
where
    K: Clone + Default + Eq + PartialEq + Hash + Send + Sync + 'static,
    V: Clone + Default + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            //data : Arc::new(RwLock::new(HashMap::new())),
            data: HashMap::new(),
        }
    }

    pub async fn fetch<F, Fut>(&mut self, key: K, fx: F) -> FetchState<V>
    where
        F: FnOnce(K) -> Fut + Send + 'static,
        Fut: Future<Output = Result<V>> + Send + 'static,
    {
        if let Some(x) = self.data.get(&key) {
            return x.clone();
        }

        let lock: Arc<RwLock<Option<V>>> = Arc::new(RwLock::new(None));

        let lock_clone = lock.clone();
        self.data.insert(key.clone(), Fetching(lock.clone()));

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();

        tokio::task::spawn(async move {
            let mut write_guard = lock_clone.write().await;
            if let Err(e) = tx.send(true) {
                error!("{}", e)
            }
            let fetched_data = fx(key).await;

            match fetched_data {
                Ok(v) => {
                    *write_guard = Some(v);
                }
                _ => {
                    *write_guard = None;
                }
            }
        });

        if let Err(e) = rx.await {
            error!("{}", e)
        };
        Fetching(lock)
    }

    pub async fn get<F, Fut>(&mut self, key: K, fx: F) -> Result<Option<V>>
    where
        F: FnOnce(K) -> Fut + Send + 'static,
        Fut: Future<Output = Result<V>> + Send + 'static,
    {
        match self.fetch(key.clone(), fx).await {
            Fetching(lock) => {
                let ret = lock.read().await;
                Ok(ret.clone())
            }
            FetchState::Ready(v) => Ok(Some(v)),
        }
    }
}
