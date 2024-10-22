use alloy_primitives::Address;
use defi_entities::config::StrategyConfig;
use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct BackrunConfigSection {
    pub backrun_strategy: BackrunConfig,
}

#[derive(Clone, Deserialize, Debug)]
pub struct BackrunConfig {
    eoa: Option<Address>,
    smart: bool,
}

impl StrategyConfig for BackrunConfig {
    fn eoa(&self) -> Option<Address> {
        self.eoa
    }
}

impl BackrunConfig {
    pub(crate) fn smart(&self) -> bool {
        self.smart
    }
}

impl Default for BackrunConfig {
    fn default() -> Self {
        Self { eoa: None, smart: true }
    }
}
