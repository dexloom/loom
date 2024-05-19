use alloy_primitives::{Address, Bytes, U256, U64};
use alloy_provider::{Network, Provider};
use alloy_rpc_types::BlockNumberOrTag;
use alloy_transport::{Transport, TransportResult};

use crate::{AnvilDebugProvider, DebugProviderExt};

pub(crate) fn convert_u64(r: U64) -> u64 {
    r.to::<u64>()
}

pub trait AnvilProviderExt<T, N>
    where
        N: Network,
        T: Transport + Clone,
{
    fn snapshot(&self) -> impl std::future::Future<Output=TransportResult<u64>> + Send;
    fn revert(&self, snap_id: u64) -> impl std::future::Future<Output=TransportResult<bool>> + Send;


    fn mine(&self) -> impl std::future::Future<Output=TransportResult<u64>> + Send;

    fn set_automine(&self, to_mine: bool) -> impl std::future::Future<Output=TransportResult<()>> + Send;


    fn set_code(&self, address: Address, code: Bytes) -> impl std::future::Future<Output=TransportResult<()>> + Send;

    fn set_balance(&self, address: Address, balance: U256) -> impl std::future::Future<Output=TransportResult<()>> + Send;
}


impl<PN, PA, TN, TA, N> AnvilProviderExt<TA, N> for AnvilDebugProvider<PN, PA, TN, TA, N>
    where
        N: Network,
        TN: Transport + Clone,
        TA: Transport + Clone,
        PN: Provider<TN, N> + Send + Sync + Clone + 'static,
        PA: Provider<TA, N> + Send + Sync + Clone + 'static
{
    fn snapshot(&self) -> impl std::future::Future<Output=TransportResult<u64>> + Send {
        self.anvil().client().request("evm_snapshot", ()).map_resp(|x| convert_u64(x))
    }
    fn revert(&self, snap_id: u64) -> impl std::future::Future<Output=TransportResult<bool>> + Send {
        self.anvil().client().request("evm_revert", (U64::from(snap_id), ))
    }

    fn set_automine(&self, to_mine: bool) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.anvil().client().request("evm_setAutomine", (to_mine, ))
    }

    fn mine(&self) -> impl std::future::Future<Output=TransportResult<u64>> + Send {
        self.anvil().client().request("evm_mine", ()).map_resp(|x| convert_u64(x))
    }

    fn set_code(&self, address: Address, code: Bytes) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.anvil().client().request("anvil_setCode", (address, code))
    }

    fn set_balance(&self, address: Address, balance: U256) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.anvil().client().request("anvil_setBalance", (address, balance))
    }
}


#[cfg(test)]
mod test {
    use std::sync::Arc;

    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::ClientBuilder;
    use env_logger::Env as EnvLog;
    use eyre::Result;
    use url;

    use super::*;

    #[tokio::test]
    async fn test() -> Result<()> {
        std::env::set_var("RUST_LOG", "debug");
        std::env::set_var("RUST_BACKTRACE", "1");
        let test_node_url = std::env::var("TEST_NODE_URL").unwrap_or("http://localhost:8545".to_string());
        let node_url = std::env::var("NODE_URL").unwrap_or("http://falcon.loop:8008/rpc".to_string());

        env_logger::init_from_env(EnvLog::default().default_filter_or("debug"));
        let test_node_url = url::Url::parse(test_node_url.as_str())?;
        let node_url = url::Url::parse(node_url.as_str())?;

        let client_anvil = ClientBuilder::default().http(test_node_url).boxed();
        let provider_anvil = ProviderBuilder::new().on_client(client_anvil).boxed();

        let client_node = ClientBuilder::default().http(node_url).boxed();
        let provider_node = ProviderBuilder::new().on_client(client_node).boxed();


        let provider = AnvilDebugProvider::new(provider_node, provider_anvil, BlockNumberOrTag::Number(10));

        let client = Arc::new(provider);


        let snap = client.snapshot().await?;
        let revert_result = client.revert(snap).await?;
        client.set_automine(false).await?;
        let mine_result = client.mine().await;
        client.set_automine(true).await?;
        //let reset_result = client.reset().await?;


        Ok(())
    }
}
