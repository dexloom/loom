use alloy_primitives::{BlockNumber, U64};
use alloy_provider::{Network, Provider, RootProvider};
use alloy_provider::ext::DebugApi;
use alloy_rpc_client::RpcCall;
use alloy_rpc_types::{BlockNumberOrTag, TransactionRequest};
use alloy_rpc_types_trace::geth::{GethDebugTracingCallOptions, GethTrace};
use alloy_transport::{BoxTransport, Transport, TransportResult};
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct AnvilDebugProvider<T, N>
{
    _node: RootProvider<T, N>,
    _anvil: RootProvider<T, N>,
    block_number: BlockNumberOrTag,
}

impl<T, N> AnvilDebugProvider<T, N>
    where
        T: Transport + Clone,
        N: Network
{
    pub fn new(_node: RootProvider<T, N>, _anvil: RootProvider<T, N>, block_number: BlockNumberOrTag) -> Self {
        Self { _node, _anvil, block_number }
    }
}


#[async_trait]
impl<T, N> Provider<T, N> for AnvilDebugProvider<T, N>
    where
        T: Transport + Clone,
        N: Network
{
    #[inline(always)]
    fn root(&self) -> &RootProvider<T, N> {
        self._node.root()
    }


    fn get_block_number(&self) -> RpcCall<T, (), U64, BlockNumber> {
        self._anvil.get_block_number()
    }
}


#[async_trait]
pub trait DebugProviderExt
{
    async fn geth_debug_trace_call(&self, tx: TransactionRequest, block: BlockNumberOrTag, trace_options: GethDebugTracingCallOptions) -> TransportResult<GethTrace>;
}

#[async_trait]
impl DebugProviderExt for RootProvider<BoxTransport> {
    async fn geth_debug_trace_call(&self, tx: TransactionRequest, block: BlockNumberOrTag, trace_options: GethDebugTracingCallOptions) -> TransportResult<GethTrace> {
        self.debug_trace_call(tx, block, trace_options).await
    }
}

#[async_trait]
impl<T, N> DebugProviderExt for AnvilDebugProvider<T, N>
    where
        T: Transport + Clone, N: Network
{
    async fn geth_debug_trace_call(&self, tx: TransactionRequest, block: BlockNumberOrTag, trace_options: GethDebugTracingCallOptions) -> TransportResult<GethTrace> {
        self._node.debug_trace_call(tx, block, trace_options).await
    }
}


#[cfg(test)]
mod test {
    use std::sync::Arc;

    use alloy_primitives::{Address, U256};
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

        let block_number = client.get_block_number().await?;

        let contract: Address = "0x90e7a93e0a6514cb0c84fc7acc1cb5c0793352d2".parse()?;
        let location: U256 = U256::from(0);

        let cell0 = client.get_storage_at(contract, location, BlockNumberOrTag::Latest.into()).await?;
        println!("{} {}", block_number, cell0);

        match client.geth_debug_trace_call(TransactionRequest::default(), BlockNumberOrTag::Latest, GethDebugTracingCallOptions::default()).await {
            Ok(_) => {
                println!("Ok")
            }
            Err(e) => {
                println!("Error :{}", e)
            }
        }


        Ok(())
    }
}
