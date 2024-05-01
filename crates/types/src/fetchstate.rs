use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub enum FetchState<T: Clone> {
    Fetching(Arc<RwLock<Option<T>>>),
    Ready(T),
}
