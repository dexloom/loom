use alloy::{
    primitives::{Address, Bytes, B256, U256, U64},
    providers::{network::Ethereum, Network, Provider, RootProvider},
    transports::TransportResult,
};

use crate::AnvilDebugProvider;

pub(crate) fn convert_u64(r: U64) -> u64 {
    r.to::<u64>()
}

pub trait AnvilProviderExt<N>
where
    N: Network,
{
    fn snapshot(&self) -> impl std::future::Future<Output = TransportResult<u64>> + Send;
    fn revert(&self, snap_id: u64) -> impl std::future::Future<Output = TransportResult<bool>> + Send;

    fn mine(&self) -> impl std::future::Future<Output = TransportResult<u64>> + Send;

    fn set_automine(&self, to_mine: bool) -> impl std::future::Future<Output = TransportResult<()>> + Send;

    fn set_code(&self, address: Address, code: Bytes) -> impl std::future::Future<Output = TransportResult<()>> + Send;

    fn set_balance(&self, address: Address, balance: U256) -> impl std::future::Future<Output = TransportResult<()>> + Send;

    //fn get_storage(&self, address: Address, cell: U256) -> impl std::future::Future<Output=TransportResult<U256>> + Send;
    fn set_storage(&self, address: Address, cell: B256, value: B256) -> impl std::future::Future<Output = TransportResult<bool>> + Send;
}

impl<PN, PA, N> AnvilProviderExt<N> for AnvilDebugProvider<PN, PA, N>
where
    N: Network,
    PN: Provider<N> + Send + Sync + Clone + 'static,
    PA: Provider<N> + Send + Sync + Clone + 'static,
{
    fn snapshot(&self) -> impl std::future::Future<Output = TransportResult<u64>> + Send {
        self.anvil().client().request("evm_snapshot", ()).map_resp(convert_u64)
    }
    fn revert(&self, snap_id: u64) -> impl std::future::Future<Output = TransportResult<bool>> + Send {
        self.anvil().client().request("evm_revert", (U64::from(snap_id),))
    }

    fn mine(&self) -> impl std::future::Future<Output = TransportResult<u64>> + Send {
        self.anvil().client().request("evm_mine", ()).map_resp(convert_u64)
    }

    fn set_automine(&self, to_mine: bool) -> impl std::future::Future<Output = TransportResult<()>> + Send {
        self.anvil().client().request("evm_setAutomine", (to_mine,))
    }

    fn set_code(&self, address: Address, code: Bytes) -> impl std::future::Future<Output = TransportResult<()>> + Send {
        self.anvil().client().request("anvil_setCode", (address, code))
    }

    fn set_balance(&self, address: Address, balance: U256) -> impl std::future::Future<Output = TransportResult<()>> + Send {
        self.anvil().client().request("anvil_setBalance", (address, balance))
    }

    fn set_storage(&self, address: Address, cell: B256, value: B256) -> impl std::future::Future<Output = TransportResult<bool>> + Send {
        self.anvil().client().request("anvil_setStorageAt", (address, cell, value))
    }
    /*    fn get_storage(&self, address: Address, cell: U256) -> impl std::future::Future<Output=TransportResult<U256>> + Send {
        self.anvil().client().request("anvil_getStorageAt", (address, cell))
    }*/
}

impl AnvilProviderExt<Ethereum> for RootProvider<Ethereum> {
    fn snapshot(&self) -> impl std::future::Future<Output = TransportResult<u64>> + Send {
        self.client().request("evm_snapshot", ()).map_resp(convert_u64)
    }
    fn revert(&self, snap_id: u64) -> impl std::future::Future<Output = TransportResult<bool>> + Send {
        self.client().request("evm_revert", (U64::from(snap_id),))
    }

    fn mine(&self) -> impl std::future::Future<Output = TransportResult<u64>> + Send {
        self.client().request("evm_mine", ()).map_resp(convert_u64)
    }

    fn set_automine(&self, to_mine: bool) -> impl std::future::Future<Output = TransportResult<()>> + Send {
        self.client().request("evm_setAutomine", (to_mine,))
    }

    fn set_code(&self, address: Address, code: Bytes) -> impl std::future::Future<Output = TransportResult<()>> + Send {
        self.client().request("anvil_setCode", (address, code))
    }

    fn set_balance(&self, address: Address, balance: U256) -> impl std::future::Future<Output = TransportResult<()>> + Send {
        self.client().request("anvil_setBalance", (address, balance))
    }

    fn set_storage(&self, address: Address, cell: B256, value: B256) -> impl std::future::Future<Output = TransportResult<bool>> + Send {
        self.client().request("anvil_setStorageAt", (address, cell, value))
    }
    /*    fn get_storage(&self, address: Address, cell: U256) -> impl std::future::Future<Output=TransportResult<U256>> + Send {
        self.anvil().client().request("anvil_getStorageAt", (address, cell))
    }*/
}

#[cfg(test)]
mod test {
    use alloy::node_bindings::Anvil;
    use alloy::primitives::{B256, U256};
    use alloy::rpc::types::BlockNumberOrTag;
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::ClientBuilder;
    use env_logger::Env as EnvLog;
    use eyre::Result;
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn test_storage() -> Result<()> {
        let _ = env_logger::try_init_from_env(EnvLog::default().default_filter_or("info"));

        let anvil = Anvil::new().try_spawn()?;

        let client_anvil = ClientBuilder::default().http(anvil.endpoint_url());

        let provider = ProviderBuilder::new().disable_recommended_fillers().on_client(client_anvil);

        let address: Address = Address::repeat_byte(0x12);

        if let Err(e) = provider.set_storage(address, B256::from(U256::from(1)), B256::from(U256::from(2))).await {
            panic!("{}", e);
        }

        let value = provider.get_storage_at(address, U256::from(1)).await?;
        if value != U256::from(2) {
            panic!("Incorrect value {}", value);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_anvil_commands() -> Result<()> {
        let _ = env_logger::try_init_from_env(EnvLog::default().default_filter_or("info"));
        let anvil = Anvil::new().try_spawn()?;

        let node_url = std::env::var("MAINNET_HTTP").unwrap().to_string();

        let test_node_url = anvil.endpoint_url();

        let node_url = url::Url::parse(node_url.as_str())?;

        let client_anvil = ClientBuilder::default().http(test_node_url);
        let provider_anvil = ProviderBuilder::new().disable_recommended_fillers().on_client(client_anvil);

        let client_node = ClientBuilder::default().http(node_url);
        let provider_node = ProviderBuilder::new().disable_recommended_fillers().on_client(client_node);

        let provider = AnvilDebugProvider::new(provider_node, provider_anvil, BlockNumberOrTag::Number(10));

        let client = Arc::new(provider);

        let snap = client.snapshot().await?;
        let _ = client.revert(snap).await?;
        client.set_automine(false).await?;
        let _ = client.mine().await;
        client.set_automine(true).await?;

        Ok(())
    }
}
