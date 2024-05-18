use std::collections::HashMap;

use alloy_primitives::{Address, TxHash};
use eyre::Result;
use serde::Deserialize;
use tokio::fs;

use defi_entities::PoolClass;

#[derive(Deserialize, Debug)]
pub struct TestConfig {
    pub settings: Settings,
    pub pools: HashMap<String, PoolConfig>,
    pub txs: HashMap<String, TransactionConfig>,
    pub tokens: HashMap<String, TokenConfig>,
}

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
    pub decimals: Option<i32>,
    pub basic: Option<bool>,
    pub middle: Option<bool>,
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
    fn test_desrialization() {
        let cfg = r#"
[settings]
block = 19101579
coinbase = "0x1dd35b4da6534230ff53048f7477f17f7f4e7a70"
multicaller = "0x3dd35b4da6534230ff53048f7477f17f7f4e7a70"
skip_default = false

[pools]
a = { address = "0x2dd35b4da6534230ff53048f7477f17f7f4e7a70", class = "uniswap2" }

[txs]
tx_1 = { hash = "0xf9fb98fe76dc5f4e836cdc3d80cd7902150a8609c617064f1447c3980fd6776b", send = "mempool" }
tx_2 = { hash = "0x1ec982c2d4eb5475192b26f7208b797328eab88f8e5be053f797f74bcb87a20c", send = "mempool" }

[tokens]
weth = { address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", symbol = "WETH", decimals = 18, basic = true, middle = false }
        "#;
        let config: TestConfig = toml::from_str(&cfg).unwrap();
    }
}