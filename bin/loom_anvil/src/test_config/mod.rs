use std::collections::HashMap;

use alloy_primitives::{Address, TxHash};
use eyre::Result;
use serde::Deserialize;
use tokio::fs;

use loom::types::entities::PoolClass;

#[derive(Deserialize, Debug)]
pub struct TestConfig {
    pub modules: Modules,
    pub settings: Settings,
    pub pools: HashMap<String, PoolConfig>,
    pub txs: HashMap<String, TransactionConfig>,
    pub tokens: HashMap<String, TokenConfig>,
    pub assertions: AssertionsConfig,
}

fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

#[allow(dead_code)]
#[derive(Default, Deserialize, Debug, Clone)]
pub struct AssertionsConfig {
    pub swaps_encoded: Option<usize>,
    pub swaps_ok: Option<usize>,
    pub best_profit_eth: Option<f64>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Modules {
    #[serde(default = "default_true")]
    pub price: bool,
    #[serde(default = "default_true")]
    pub signer: bool,
    #[serde(default = "default_true")]
    pub encoder: bool,
    #[serde(default = "default_true")]
    pub arb_path_merger: bool,
    #[serde(default = "default_true")]
    pub same_path_merger: bool,
    #[serde(default = "default_true")]
    pub diff_path_merger: bool,
    #[serde(default)]
    pub arb_block: bool,
    #[serde(default = "default_true")]
    pub arb_mempool: bool,
    #[serde(default = "default_false")]
    pub flashbots: bool,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Settings {
    pub block: u64,
    pub coinbase: Option<Address>,
    pub multicaller: Option<Address>,
}

#[derive(Deserialize, Debug)]
pub struct PoolConfig {
    pub address: Address,
    pub class: PoolClass,
}

#[derive(Deserialize, Debug)]
pub struct TransactionConfig {
    pub hash: TxHash,
    pub send: String,
}

#[derive(Deserialize, Debug)]
pub struct TokenConfig {
    pub address: Address,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub decimals: Option<u8>,
    pub basic: Option<bool>,
    pub middle: Option<bool>,
    pub price: Option<f64>,
}

impl TestConfig {
    pub async fn from_file(filename: String) -> Result<TestConfig> {
        let toml_content = fs::read_to_string(filename.as_str()).await?;
        let config: TestConfig = toml::from_str(&toml_content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod test {
    use crate::test_config::TestConfig;

    #[test]
    fn test_deserialization() {
        let cfg = r#"
[settings]
block = 19101579
coinbase = "0x1dd35b4da6534230ff53048f7477f17f7f4e7a70"
multicaller = "0x3dd35b4da6534230ff53048f7477f17f7f4e7a70"
skip_default = false

[modules]

[pools]
a = { address = "0x2dd35b4da6534230ff53048f7477f17f7f4e7a70", class = "uniswap2" }

[txs]
tx_1 = { hash = "0xf9fb98fe76dc5f4e836cdc3d80cd7902150a8609c617064f1447c3980fd6776b", send = "mempool" }
tx_2 = { hash = "0x1ec982c2d4eb5475192b26f7208b797328eab88f8e5be053f797f74bcb87a20c", send = "mempool" }

[tokens]
weth = { address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", symbol = "WETH", decimals = 18, basic = true, middle = false }

[assertions]
swaps_encoded = 14
swaps_ok = 11
best_profit_eth = 181.37
        "#;
        let config: TestConfig = toml::from_str(cfg).unwrap();
        assert_eq!(config.settings.block, 19101579);
        assert_eq!(config.assertions.swaps_encoded.unwrap_or_default(), 14);
        assert_eq!(config.assertions.swaps_ok.unwrap_or_default(), 11);
        assert_eq!(config.assertions.best_profit_eth.unwrap_or_default(), 181.37);
    }
}
