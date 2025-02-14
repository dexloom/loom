use std::marker::PhantomData;
use std::sync::Arc;

use alloy::eips::BlockId;
use alloy::{
    network::Ethereum,
    node_bindings::{Anvil, AnvilInstance},
    primitives::{BlockHash, BlockNumber, U64},
    providers::{ext::DebugApi, Network, Provider, ProviderBuilder, ProviderCall, RootProvider},
    rpc::{
        client::{NoParams, WsConnect},
        types::trace::geth::{GethDebugTracingCallOptions, GethDebugTracingOptions, GethTrace, TraceResult},
        types::{BlockNumberOrTag, BlockTransactionsKind, TransactionRequest},
    },
    transports::TransportResult,
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use k256::SecretKey;

//use crate::HttpCachedTransport;

#[derive(Clone, Debug)]
pub struct AnvilDebugProvider<PN, PA, N>
where
    N: Network,
    PN: Provider<N> + Send + Sync + Clone + 'static,
    PA: Provider<N> + Send + Sync + Clone + 'static,
{
    _node: PN,
    _anvil: PA,
    _anvil_instance: Option<Arc<AnvilInstance>>,
    block_number: BlockNumberOrTag,
    _n: PhantomData<N>,
}

pub struct AnvilDebugProviderFactory {}

pub type AnvilDebugProviderType = AnvilDebugProvider<RootProvider<Ethereum>, RootProvider<Ethereum>, Ethereum>;

impl AnvilDebugProviderFactory {
    pub async fn from_node_on_block(node_url: String, block: BlockNumber) -> Result<AnvilDebugProviderType> {
        let node_ws = WsConnect::new(node_url.clone());
        let node_provider = ProviderBuilder::new().disable_recommended_fillers().on_ws(node_ws).await?;

        let anvil = Anvil::new().fork_block_number(block).fork(node_url.clone()).chain_id(1).arg("--disable-console-log").spawn();

        //let anvil_layer = AnvilLayer::from(anvil.clone());
        let anvil_url = anvil.ws_endpoint_url();
        let anvil_ws = WsConnect::new(anvil_url.clone());

        let anvil_provider = ProviderBuilder::new().disable_recommended_fillers().on_ws(anvil_ws).await?;

        let curblock = anvil_provider.get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes).await?;

        match curblock {
            Some(curblock) => {
                if curblock.header.number != block {
                    return Err(eyre!("INCORRECT_BLOCK_NUMBER"));
                }
            }
            _ => {
                return Err(eyre!("CANNOT_GET_BLOCK"));
            }
        }

        let ret = AnvilDebugProvider {
            _node: node_provider,
            _anvil: anvil_provider,
            _anvil_instance: Some(Arc::new(anvil)),
            block_number: BlockNumberOrTag::Number(block),
            _n: PhantomData::<Ethereum>,
        };

        let curblock = ret._anvil.get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes).await?;

        match curblock {
            Some(curblock) => {
                if curblock.header.number != block {
                    return Err(eyre!("INCORRECT_BLOCK_NUMBER"));
                }
            }
            _ => {
                return Err(eyre!("CANNOT_GET_BLOCK"));
            }
        }

        Ok(ret)
    }
}

impl<PN, PA, N> AnvilDebugProvider<PN, PA, N>
where
    N: Network,
    PA: Provider<N> + Send + Sync + Clone + 'static,
    PN: Provider<N> + Send + Sync + Clone + 'static,
{
    pub fn new(_node: PN, _anvil: PA, block_number: BlockNumberOrTag) -> Self {
        Self { _node, _anvil, _anvil_instance: None, block_number, _n: PhantomData }
    }

    pub fn node(&self) -> &PN {
        &self._node
    }
    pub fn anvil(&self) -> &PA {
        &self._anvil
    }

    pub fn privkey(&self) -> Result<SecretKey> {
        match &self._anvil_instance {
            Some(anvil) => Ok(anvil.clone().keys()[0].clone()),
            _ => Err(eyre!("NO_ANVIL_INSTANCE")),
        }
    }
}

/*
impl<PN, PA, TN, TA, N> Provider<TA, N> for AnvilDebugProvider<PN, PA, TN, TA, N>
    where
        TN: Transport + Clone,
        TA: Transport + Clone,
        N: Network,
        PN: Provider<TN, N> + Send + Sync + Clone + 'static,
        PA: Provider<TA, N> + Send + Sync + Clone + 'static
{
    #[inline(always)]
    fn root(&self) -> &RootProvider<TA, N> {
        self._anvil.root()
    }


    fn get_block_number(&self) -> RpcCall<TA, (), u64> {
        self._anvil.get_block_number()
    }
}

 */

impl<PN, PA> Provider<Ethereum> for AnvilDebugProvider<PN, PA, Ethereum>
where
    PN: Provider<Ethereum> + Send + Sync + Clone + 'static,
    PA: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    #[inline(always)]
    fn root(&self) -> &RootProvider<Ethereum> {
        self._anvil.root()
    }

    #[allow(clippy::type_complexity)]
    fn get_block_number(&self) -> ProviderCall<NoParams, U64, BlockNumber> {
        self._anvil.get_block_number()
    }
}

#[async_trait]
pub trait DebugProviderExt<N = Ethereum> {
    async fn geth_debug_trace_call(
        &self,
        tx: TransactionRequest,
        block: BlockId,
        trace_options: GethDebugTracingCallOptions,
    ) -> TransportResult<GethTrace>;
    async fn geth_debug_trace_block_by_number(
        &self,
        block: BlockNumberOrTag,
        trace_options: GethDebugTracingOptions,
    ) -> TransportResult<Vec<TraceResult>>;
    async fn geth_debug_trace_block_by_hash(
        &self,
        block: BlockHash,
        trace_options: GethDebugTracingOptions,
    ) -> TransportResult<Vec<TraceResult>>;
}

#[async_trait]
impl<N> DebugProviderExt<N> for RootProvider<Ethereum>
where
    N: Network,
{
    async fn geth_debug_trace_call(
        &self,
        tx: TransactionRequest,
        block: BlockId,
        trace_options: GethDebugTracingCallOptions,
    ) -> TransportResult<GethTrace> {
        self.debug_trace_call(tx, block, trace_options).await
    }
    async fn geth_debug_trace_block_by_number(
        &self,
        block: BlockNumberOrTag,
        trace_options: GethDebugTracingOptions,
    ) -> TransportResult<Vec<TraceResult>> {
        self.debug_trace_block_by_number(block, trace_options).await
    }
    async fn geth_debug_trace_block_by_hash(
        &self,
        block: BlockHash,
        trace_options: GethDebugTracingOptions,
    ) -> TransportResult<Vec<TraceResult>> {
        self.debug_trace_block_by_hash(block, trace_options).await
    }
}

#[async_trait]
impl<PN, PA, N> DebugProviderExt<N> for AnvilDebugProvider<PN, PA, N>
where
    N: Network,
    PN: Provider<N> + Send + Sync + Clone + 'static,
    PA: Provider<N> + Send + Sync + Clone + 'static,
{
    async fn geth_debug_trace_call(
        &self,
        tx: TransactionRequest,
        block: BlockId,
        trace_options: GethDebugTracingCallOptions,
    ) -> TransportResult<GethTrace> {
        let block = match block {
            BlockId::Hash(hash) => BlockId::Hash(hash),
            BlockId::Number(number) => match number {
                BlockNumberOrTag::Number(number) => BlockId::Number(BlockNumberOrTag::Number(number)),
                BlockNumberOrTag::Latest => BlockId::Number(self.block_number),
                _ => block,
            },
        };
        self._node.debug_trace_call(tx, block, trace_options).await
    }
    async fn geth_debug_trace_block_by_number(
        &self,
        block: BlockNumberOrTag,
        trace_options: GethDebugTracingOptions,
    ) -> TransportResult<Vec<TraceResult>> {
        self._node.debug_trace_block_by_number(block, trace_options).await
    }
    async fn geth_debug_trace_block_by_hash(
        &self,
        block: BlockHash,
        trace_options: GethDebugTracingOptions,
    ) -> TransportResult<Vec<TraceResult>> {
        self._node.debug_trace_block_by_hash(block, trace_options).await
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy::primitives::{Address, U256};
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::ClientBuilder;
    use env_logger::Env as EnvLog;
    use eyre::Result;
    use tracing::{debug, error};

    #[tokio::test]
    async fn test_debug_trace_call() -> Result<()> {
        let _ = env_logger::try_init_from_env(
            EnvLog::default().default_filter_or("info,hyper_util=off,alloy_transport_http=off,alloy_rpc_client=off,reqwest=off"),
        );

        let node_url = url::Url::parse(std::env::var("MAINNET_HTTP")?.as_str())?;

        let provider_anvil =
            ProviderBuilder::new().on_anvil_with_config(|x| x.chain_id(1).fork(node_url.clone()).fork_block_number(20322777));

        let client_node = ClientBuilder::default().http(node_url);

        let provider_node = ProviderBuilder::new().disable_recommended_fillers().on_client(client_node);

        let provider = AnvilDebugProvider::new(provider_node, provider_anvil, BlockNumberOrTag::Number(10));

        let client = provider;

        let block_number = client.get_block_number().await?;

        let contract: Address = "0x90e7a93e0a6514cb0c84fc7acc1cb5c0793352d2".parse()?;
        let location: U256 = U256::from(0);

        let cell0 = client.get_storage_at(contract, location).block_id(BlockNumberOrTag::Latest.into()).await?;
        debug!("{} {}", block_number, cell0);

        match client
            .geth_debug_trace_call(
                TransactionRequest::default(),
                BlockId::Number(BlockNumberOrTag::Latest),
                GethDebugTracingCallOptions::default(),
            )
            .await
        {
            Ok(trace) => {
                debug!("Ok {:?}", trace);
            }
            Err(e) => {
                error!("Error :{}", e);
                panic!("DEBUG_TRACE_CALL_FAILED");
            }
        }

        Ok(())
    }
}
