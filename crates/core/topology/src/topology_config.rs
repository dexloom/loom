use eyre::Result;
use loom_broadcast_flashbots::client::RelayConfig;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use strum_macros::Display;

#[derive(Clone, Debug, Deserialize)]
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
    #[serde(rename = "ws")]
    Ws,
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "ipc")]
    Ipc,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct InfluxDbConfig {
    pub url: String,
    pub database: String,
    pub tags: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ClientConfig {
    pub url: String,
    pub node: NodeType,
    pub transport: TransportType,
    pub db_path: Option<String>,
    pub exex: Option<String>,
    // #[serde(skip)]
    // pub provider: Option<P>,
    // _n: PhantomData<N>,
}

/*
impl<P, N> ClientConfig<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pub fn client(&self) -> Option<&P> {
        self.provider.as_ref()
    }
}

 */
/*
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ClientConfig {
    //String(String),
    Params(ClientConfigParams),
}

impl ClientConfig {
    pub fn url(&self) -> String {
        match &self {
            Self::String(s) => s.clone(),
            ClientConfig::Params(p) => p.url.clone(),
        }
    }

    pub fn config_params(&self) -> ClientConfigParams {
        match self {
            ClientConfig::String(s) => ClientConfigParams { url: s.clone(), ..ClientConfigParams::default() },
            ClientConfig::Params(p) => p.clone(),
        }
    }
}

 */

#[derive(Clone, Debug, Deserialize)]
pub struct EnvSingerConfig {
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum SignersConfig {
    #[serde(rename = "env")]
    Env(EnvSingerConfig),
}

#[derive(Clone, Debug, Deserialize)]
pub struct PreloaderConfig {
    pub client: Option<String>,
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
    pub encoder: Option<String>,
    pub signers: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SwapStepEncoderConfig {
    pub address: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum EncoderConfig {
    #[serde(rename = "swapstep")]
    SwapStep(SwapStepEncoderConfig),
}

#[derive(Clone, Debug, Deserialize)]
pub struct BlockchainClientConfig {
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
    pub client: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ExExClientConfig {
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlashbotsRelayConfig {
    id: u16,
    name: String,
    url: String,
    no_sign: Option<bool>,
}

impl From<FlashbotsRelayConfig> for RelayConfig {
    fn from(config: FlashbotsRelayConfig) -> Self {
        RelayConfig { id: config.id, name: config.name, url: config.url, no_sign: config.no_sign }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlashbotsBroadcasterConfig {
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
    pub client: Option<String>,
    pub smart: Option<bool>,
    pub relays: Option<Vec<FlashbotsRelayConfig>>,
}

impl FlashbotsBroadcasterConfig {
    pub fn relays(&self) -> Vec<RelayConfig> {
        self.relays.as_ref().map(|relays| relays.iter().map(|r| r.clone().into()).collect()).unwrap_or_default()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum BroadcasterConfig {
    #[serde(rename = "flashbots")]
    Flashbots(FlashbotsBroadcasterConfig),
}

#[derive(Clone, Debug, Deserialize)]
pub struct EvmEstimatorConfig {
    pub client: Option<String>,
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
    pub encoder: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GethEstimatorConfig {
    pub client: Option<String>,
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
    pub encoder: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum EstimatorConfig {
    #[serde(rename = "evm")]
    Evm(EvmEstimatorConfig),
    #[serde(rename = "geth")]
    Geth(GethEstimatorConfig),
}

#[derive(Clone, Debug, Deserialize)]
pub struct PoolsConfig {
    #[serde(rename = "bc")]
    pub blockchain: Option<String>,
    pub client: Option<String>,
    pub history: bool,
    pub new: bool,
    pub protocol: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WebserverConfig {
    pub host: String,
}

impl Default for WebserverConfig {
    fn default() -> Self {
        WebserverConfig { host: "127.0.0.1:3333".to_string() }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActorConfig {
    pub broadcaster: Option<HashMap<String, BroadcasterConfig>>,
    pub node: Option<HashMap<String, BlockchainClientConfig>>,
    pub node_exex: Option<HashMap<String, ExExClientConfig>>,
    pub mempool: Option<HashMap<String, BlockchainClientConfig>>,
    pub price: Option<HashMap<String, BlockchainClientConfig>>,
    pub pools: Option<HashMap<String, PoolsConfig>>,
    pub noncebalance: Option<HashMap<String, BlockchainClientConfig>>,
    pub estimator: Option<HashMap<String, EstimatorConfig>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TopologyConfig {
    pub influxdb: Option<InfluxDbConfig>,
    pub clients: HashMap<String, ClientConfig>,
    pub blockchains: HashMap<String, BlockchainConfig>,
    pub actors: ActorConfig,
    pub signers: HashMap<String, SignersConfig>,
    pub encoders: HashMap<String, EncoderConfig>,
    pub preloaders: Option<HashMap<String, PreloaderConfig>>,
    pub webserver: Option<WebserverConfig>,
    pub database: Option<DatabaseConfig>,
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
            Err(e) => {
                println!("Error:{e}")
            }
        }
    }
}
