use loom_types_entities::PoolClass;
use std::collections::HashMap;
use strum::IntoEnumIterator;

#[derive(Clone)]
pub struct PoolsConfig {
    is_enabled: HashMap<PoolClass, bool>,
}

impl PoolsConfig {
    pub fn new() -> Self {
        let mut is_enabled = HashMap::new();
        for pool_class in PoolClass::iter() {
            is_enabled.insert(pool_class, true);
        }

        Self { is_enabled }
    }

    pub fn disable_all() -> Self {
        let mut is_enabled = HashMap::new();
        for pool_class in PoolClass::iter() {
            is_enabled.insert(pool_class, false);
        }

        Self { is_enabled }
    }

    pub fn enable(&mut self, pool_class: PoolClass) -> Self {
        *self.is_enabled.entry(pool_class).or_insert(true) = true;
        Self { is_enabled: self.is_enabled.clone() }
    }

    pub fn disable(&mut self, pool_class: PoolClass) -> Self {
        *self.is_enabled.entry(pool_class).or_insert(false) = false;
        Self { is_enabled: self.is_enabled.clone() }
    }

    pub fn is_enabled(&self, pool_class: PoolClass) -> bool {
        match self.is_enabled.get(&pool_class) {
            None => false,
            Some(val) => *val,
        }
    }
}

impl Default for PoolsConfig {
    fn default() -> Self {
        PoolsConfig::new()
    }
}
