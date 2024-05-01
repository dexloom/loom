use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

use eyre::{eyre, Result};
use tokio::sync::RwLock;

use crate::FetchState::Fetching;

#[derive(Debug, Clone)]
pub enum FetchState<T: Clone> {
    Fetching(Arc<RwLock<Option<T>>>),
    Ready(T),
}


pub struct DataFetcher<K, V>
    where
        K: Clone + Eq + PartialEq + Hash + Send + Sync + 'static,
        V: Clone + Default + Send + Sync + 'static,
//F : FnOnce(K) -> Fut + Send + Sync,
//Fut : Future<Output=Result<V>> + Send,
{
    data: HashMap<K, FetchState<V>>,

    //data : Arc<RwLock<HashMap<K,FetchState<V>>>>,
    //is_fetching : Arc<Mutex<bool>>,
    //fx : F
}

impl<K, V, > DataFetcher<K, V>
    where
        K: Clone + Eq + PartialEq + Hash + Send + Sync + 'static,
        V: Clone + Default + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            //data : Arc::new(RwLock::new(HashMap::new())),
            data: HashMap::new(),
        }
    }


    async fn fetch_fx(&self, key: K) {
        //(self.fx)(key);
    }


    pub async fn fetch<F, Fut>(&mut self, key: K, fx: F) -> FetchState<V>
        where
            F: FnOnce(K) -> Fut + Send + 'static,
            Fut: Future<Output=Result<V>> + Send + 'static,

    {
        {
            match self.data.get(&key) {
                Some(x) => {
                    return x.clone();
                }
                _ => {}
            }
        }

        let lock: Arc<RwLock<Option<V>>> = Arc::new(RwLock::new(None));

        let lock_clone = lock.clone();
        self.data.insert(key.clone(), Fetching(lock.clone()));

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();

        tokio::task::spawn(async move {
            let mut write_guard = lock_clone.write().await;
            tx.send(true);
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

        rx.await;
        FetchState::Fetching(lock)
    }


    pub async fn get<F, Fut>(&mut self, key: K, fx: F) -> Result<Option<V>>
        where
            F: FnOnce(K) -> Fut + Send + 'static,
            Fut: Future<Output=Result<V>> + Send + 'static,

    {
        match self.fetch(key.clone(), fx).await {
            Fetching(lock) => {
                let ret = lock.read().await;
                Ok(ret.clone())
            }
            FetchState::Ready(v) => {
                Ok(Some(v))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn test() {
        let const_a = 5;
//        let mut df : DataFetcher<i32, i32, _ , _> = DataFetcher::new(  move |x|async move { Ok(x + const_a)} );
        let mut df: DataFetcher<i32, i32> = DataFetcher::new();

        //let df = Arc::new(RwLock::new(df));


        let a: i32 = 10;
        let b: i32 = 20;


        let r = df.get(a, move |x| async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(x + const_a)
        }).await;
        println!("{:?} {}", r, chrono::Local::now());


        let r = df.get(b, move |x| async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Err(eyre!("hmm"))
        }).await;
        println!("{:?} {}", r, chrono::Local::now());

        let r = df.get(a, move |x| async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(x + const_a)
        }).await;
        println!("{:?} {}", r, chrono::Local::now());


        /*        let r = df.write().await.get(a,move |x| async move { tokio::time::sleep(Duration::from_secs(1)).await; Ok(x + const_a)}).await;
                println!("{:?} {}", r, chrono::Local::now());
                let r = df.write().await.get(a,move |x| async move { tokio::time::sleep(Duration::from_secs(1)).await; Ok(x + const_a)}).await;
                println!("{:?} {}", r, chrono::Local::now());
                let r = df.write().await.get(b, move |x| async move { tokio::time::sleep(Duration::from_secs(1)).await; Ok(x + const_a)}).await;
                println!("{:?} {}", r, chrono::Local::now());
                let r = df.write().await.get(b, move |x| async move { tokio::time::sleep(Duration::from_secs(1)).await; Ok(x + const_a)}).await;
                println!("{:?} {}", r, chrono::Local::now());
                let r = df.write().await.get(b, move |x| async move { tokio::time::sleep(Duration::from_secs(1)).await; Ok(x + const_a)}).await;
                println!("{:?} {}", r, chrono::Local::now());


         */
    }
}