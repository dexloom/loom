use alloy::{
    primitives::{Address, B256, Bytes, U256, U64},
    providers::{Network, network::Ethereum, Provider, RootProvider},
    transports::{BoxTransport, Transport, TransportResult},
};

use crate::AnvilDebugProvider;

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

    //fn get_storage(&self, address: Address, cell: U256) -> impl std::future::Future<Output=TransportResult<U256>> + Send;
    fn set_storage(&self, address: Address, cell: B256, value: B256) -> impl std::future::Future<Output=TransportResult<bool>> + Send;
}


impl<PN, PA, TN, TA, N> AnvilProviderExt<TA, N> for AnvilDebugProvider<PN, PA, TN, TA, N>
where
    N: Network,
    TN: Transport + Clone,
    TA: Transport + Clone,
    PN: Provider<TN, N> + Send + Sync + Clone + 'static,
    PA: Provider<TA, N> + Send + Sync + Clone + 'static,
{
    fn snapshot(&self) -> impl std::future::Future<Output=TransportResult<u64>> + Send {
        self.anvil().client().request("evm_snapshot", ()).map_resp(convert_u64)
    }
    fn revert(&self, snap_id: u64) -> impl std::future::Future<Output=TransportResult<bool>> + Send {
        self.anvil().client().request("evm_revert", (U64::from(snap_id),))
    }

    fn set_automine(&self, to_mine: bool) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.anvil().client().request("evm_setAutomine", (to_mine,))
    }

    fn mine(&self) -> impl std::future::Future<Output=TransportResult<u64>> + Send {
        self.anvil().client().request("evm_mine", ()).map_resp(convert_u64)
    }

    fn set_code(&self, address: Address, code: Bytes) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.anvil().client().request("anvil_setCode", (address, code))
    }

    fn set_balance(&self, address: Address, balance: U256) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.anvil().client().request("anvil_setBalance", (address, balance))
    }

    fn set_storage(&self, address: Address, cell: B256, value: B256) -> impl std::future::Future<Output=TransportResult<bool>> + Send {
        self.anvil().client().request("anvil_setStorageAt", (address, cell, value,))
    }
    /*    fn get_storage(&self, address: Address, cell: U256) -> impl std::future::Future<Output=TransportResult<U256>> + Send {
            self.anvil().client().request("anvil_getStorageAt", (address, cell))
        }*/
}


impl AnvilProviderExt<BoxTransport, Ethereum> for RootProvider<BoxTransport>
{
    fn snapshot(&self) -> impl std::future::Future<Output=TransportResult<u64>> + Send {
        self.client().request("evm_snapshot", ()).map_resp(convert_u64)
    }
    fn revert(&self, snap_id: u64) -> impl std::future::Future<Output=TransportResult<bool>> + Send {
        self.client().request("evm_revert", (U64::from(snap_id),))
    }

    fn set_automine(&self, to_mine: bool) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.client().request("evm_setAutomine", (to_mine,))
    }

    fn mine(&self) -> impl std::future::Future<Output=TransportResult<u64>> + Send {
        self.client().request("evm_mine", ()).map_resp(convert_u64)
    }

    fn set_code(&self, address: Address, code: Bytes) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.client().request("anvil_setCode", (address, code))
    }

    fn set_balance(&self, address: Address, balance: U256) -> impl std::future::Future<Output=TransportResult<()>> + Send {
        self.client().request("anvil_setBalance", (address, balance))
    }

    fn set_storage(&self, address: Address, cell: B256, value: B256) -> impl std::future::Future<Output=TransportResult<bool>> + Send {
        self.client().request("anvil_setStorageAt", (address, cell, value,))
    }
    /*    fn get_storage(&self, address: Address, cell: U256) -> impl std::future::Future<Output=TransportResult<U256>> + Send {
            self.anvil().client().request("anvil_getStorageAt", (address, cell))
        }*/
}


#[cfg(test)]
mod test {
    use std::ops::Deref;
    use std::sync::Arc;

    use alloy::primitives::{B256, U256};
    use alloy::rpc::types::BlockNumberOrTag;
    use alloy::transports::http::Http;
    use alloy_provider::ext::AnvilApi;
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::ClientBuilder;
    use env_logger::Env as EnvLog;
    use eyre::Result;
    use reqwest::Client;
    use url;

    use super::*;

    #[tokio::test]
    async fn test_storage() -> Result<()> {
        std::env::set_var("RUST_LOG", "debug");
        std::env::set_var("RUST_BACKTRACE", "1");

        let test_node_url = std::env::var("TEST_NODE_URL").unwrap_or("http://localhost:8545".to_string());
        let test_node_url = url::Url::parse(test_node_url.as_str())?;
        let client_anvil = ClientBuilder::default().http(test_node_url).boxed();

        let provider = ProviderBuilder::new().on_client(client_anvil);

        let address: Address = Address::repeat_byte(0x12);

        if let Err(e) = provider.set_storage(address, B256::from(U256::from(1)), B256::from(U256::from(2))).await {
            panic!("{e}");
        }

        let value = provider.get_storage_at(address, U256::from(1)).await?;
        if value != U256::from(2) {
            panic!("Incorrect value {value}");
        }


        Ok(())
    }


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

    #[derive(Clone)]
    struct DynPrv<T> {
        pub provider: Arc<Box<dyn Provider<T>>>,
    }

    impl<T> Deref for DynPrv<T>
    where
        T: Transport + Clone,
    {
        type Target = Arc<Box<dyn Provider<T> + 'static>>;

        fn deref(&self) -> &Self::Target {
            &self.provider
        }
    }


    struct DynProvider {
        inner: DynPrv<BoxTransport>,
    }

    impl From<DynPrv<Http<Client>>> for DynProvider {
        fn from(value: DynPrv<Http<Client>>) -> Self {
            let provider = DynPrv { provider: Arc::new(Box::new(ProviderBuilder::new().on_provider(value.root().clone().boxed()))) };
            Self { inner: provider }
        }
    }

    impl Provider for DynProvider {
        fn root(&self) -> &RootProvider<BoxTransport, Ethereum> {
            self.inner.root()
        }
    }


    #[tokio::test]
    async fn test_dyn_provider() -> Result<()> {
        let provider = ProviderBuilder::new().with_recommended_fillers().on_anvil();

        let dyn_prv = Arc::new(Box::new(provider) as Box<dyn Provider<_>>);

        let dyn_prv = DynPrv {
            provider: dyn_prv.clone()
        };

        let dyn_prv = DynProvider::from(dyn_prv);
        
        dyn_prv.anvil_drop_all_transactions().await;

        Ok(())
    }
}
