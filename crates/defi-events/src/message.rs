use std::fmt::Debug;
use std::ops::Deref;

use chrono::Utc;

#[derive(Clone, Debug)]
pub struct Message<T> {
    pub inner: T,
    pub source: Option<String>,
    pub time: Option<chrono::DateTime<Utc>>,
}

impl<T> Message<T> {
    pub fn new(t: T) -> Self {
        Message { inner: t, source: None, time: None }
    }

    pub fn new_with_time(t: T) -> Self {
        Message { inner: t, source: None, time: Some(Utc::now()) }
    }
    pub fn new_with_source(t: T, source: String) -> Self {
        Message { inner: t, source: Some(source), time: Some(Utc::now()) }
    }

    pub fn source(&self) -> String {
        self.source.clone().unwrap_or("unknown".to_string())
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T: Debug + Clone> Deref for Message<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
