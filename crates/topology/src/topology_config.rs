use std::collections::HashMap;
use std::fs;

use alloy_provider::RootProvider;
use alloy_transport::BoxTransport;
use eyre::Result;
use serde::Deserialize;
use strum_macros::Display;

#[derive(Debug, Deserialize)]
pub struct BlockchainConfig {
    pub chain_id: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Display)]
#[strum(ascii_case_insensitive, serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    #[default]
    Geth,
    Reth,
}

#[derive(Clone, Debug, Default, Deserialize, Display)]
#[strum(ascii_case_insensitive, serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    #[default]
    Ws,
    #[serde(rename = "ws")]
    Http,
    Ipc,
}


#[derive(Clone, Debug, Default, Deserialize)]
pub struct ClientConfigParams {
    pub url: String,
    pub node: NodeType,
    pub transport: TransportType,
    pub db_path: Option<String>,
    #[serde(skip)]
    pub provider: Option<RootProvider<BoxTransport>>,
}


impl ClientConfigParams {
    pub fn client(&self) -> Option<&RootProvider<BoxTransport>> {
        self.provider.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ClientConfig {
    String(String),
    Params(ClientConfigParams),
}

impl ClientConfig {
    pub fn url(&self) -> String {
        match &self {
            Self::String(s) => s.clone(),
            ClientConfig::Params(p) => p.url.clone()
        }
    }


    pub fn config_params(&self) -> ClientConfigParams {
        match self {
            ClientConfig::String(s) => {
                ClientConfigParams {
                    url: s.clone(),
                    ..ClientConfigParams::default()
                }
            }
            ClientConfig::Params(p) => {
                p.clone()
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EnvSingerConfig {
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum SignersConfig {
    #[serde(rename = "env")]
    Env(EnvSingerConfig)
}

#[derive(Debug, Deserialize)]
pub struct PreloaderConfig {
    pub(crate) client: Option<String>,
    #[serde(rename = "bc")]
    pub(crate) blockchain: Option<String>,
    pub(crate) encoder: Option<String>,
    pub(crate) signers: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SwapStepEncoderConfig {
    pub(crate) address: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum EncoderConfig {
    #[serde(rename = "swapstep")]
    SwapStep(SwapStepEncoderConfig)
}

#[derive(Debug, Deserialize)]
pub struct BlockchainClientConfig {
    #[serde(rename = "bc")]
    pub(crate) blockchain: Option<String>,
    pub(crate) client: Option<String>,
}


#[derive(Debug, Deserialize)]
pub struct FlashbotsBroadcasaterConfig {
    #[serde(rename = "bc")]
    pub(crate) blockchain: Option<String>,
    pub(crate) client: Option<String>,
    pub(crate) smart: Option<bool>,
}


#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum BroadcasterConfig {
    #[serde(rename = "flashbots")]
    Flashbots(FlashbotsBroadcasaterConfig)
}


#[derive(Debug, Deserialize)]
pub struct EvmEstimatorConfig {
    #[serde(rename = "bc")]
    pub(crate) blockchain: Option<String>,
    //pub(crate) signers : Option<String>,
    pub(crate) encoder: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GethEstimatorConfig {
    pub(crate) client: Option<String>,
    #[serde(rename = "bc")]
    pub(crate) blockchain: Option<String>,
    //pub(crate) signers : Option<String>,
    pub(crate) encoder: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum EstimatorConfig {
    #[serde(rename = "evm")]
    Evm(EvmEstimatorConfig),
    #[serde(rename = "geth")]
    Geth(GethEstimatorConfig),
}


#[derive(Debug, Deserialize)]
pub struct PoolsConfig {
    #[serde(rename = "bc")]
    pub(crate) blockchain: Option<String>,
    pub(crate) client: Option<String>,
    pub(crate) history: bool,
    pub(crate) new: bool,
    pub(crate) protocol: bool,
}


#[derive(Debug, Deserialize)]
pub struct ActorConfig {
    pub broadcaster: HashMap<String, BroadcasterConfig>,
    pub node: HashMap<String, BlockchainClientConfig>,
    pub mempool: HashMap<String, BlockchainClientConfig>,
    pub price: HashMap<String, BlockchainClientConfig>,
    pub pools: HashMap<String, PoolsConfig>,
    pub noncebalance: HashMap<String, BlockchainClientConfig>,
    pub estimator: HashMap<String, EstimatorConfig>,
}


#[derive(Debug, Deserialize)]
pub struct TopologyConfig {
    pub clients: HashMap<String, ClientConfig>,
    pub blockchains: HashMap<String, BlockchainConfig>,
    pub actors: ActorConfig,
    pub signers: HashMap<String, SignersConfig>,
    pub encoders: HashMap<String, EncoderConfig>,
    pub preloaders: HashMap<String, PreloaderConfig>,

}


impl TopologyConfig {
    pub fn load_from_file(file_name: String) -> Result<TopologyConfig> {
        let contents = fs::read_to_string(file_name)?;
        let config: TopologyConfig = toml::from_str(&contents)?;
        Ok(config)
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_load() {
        match TopologyConfig::load_from_file("../../config.toml".to_string()) {
            Ok(c) => {
                println!("{:?}", c);
            }
            Err(e) => { println!("Error:{e}") }
        }
    }
}