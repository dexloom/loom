use crate::PoolClass;
use std::collections::HashMap;
use strum::IntoEnumIterator;

#[derive(Clone)]
pub struct PoolsLoadingConfig {
    threads: Option<usize>,
    is_enabled: HashMap<PoolClass, bool>,
}

impl PoolsLoadingConfig {
    pub fn new() -> Self {
        let mut is_enabled = HashMap::new();
        for pool_class in PoolClass::iter() {
            is_enabled.insert(pool_class, true);
        }

        Self { threads: None, is_enabled }
    }

    pub fn disable_all(self) -> Self {
        let mut is_enabled = HashMap::new();
        for pool_class in PoolClass::iter() {
            is_enabled.insert(pool_class, false);
        }

        Self { is_enabled, ..self }
    }

    pub fn enable(self, pool_class: PoolClass) -> Self {
        let mut is_enabled = self.is_enabled;
        is_enabled.insert(pool_class, true);

        Self { is_enabled, ..self }
    }

    pub fn disable(self, pool_class: PoolClass) -> Self {
        let mut is_enabled = self.is_enabled;
        is_enabled.insert(pool_class, true);

        Self { is_enabled, ..self }
    }

    pub fn is_enabled(&self, pool_class: PoolClass) -> bool {
        self.is_enabled.get(&pool_class).is_some_and(|s| *s)
    }

    pub fn with_threads(self, threads: usize) -> Self {
        Self { threads: Some(threads), ..self }
    }

    pub fn threads(&self) -> Option<usize> {
        self.threads
    }
}

impl Default for PoolsLoadingConfig {
    fn default() -> Self {
        PoolsLoadingConfig::new()
    }
}
