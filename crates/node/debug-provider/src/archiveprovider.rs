use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::anvilprovider::convert_u64;
use alloy::eips::BlockId;
use alloy::primitives::{Address, StorageValue};
use alloy::rpc::json_rpc::RpcRecv;
use alloy::rpc::types::BlockTransactionsKind;
use alloy::{
    network::Ethereum,
    primitives::{BlockNumber, Bytes, U256, U64},
    providers::{EthCall, Network, Provider, ProviderCall, RootProvider, RpcWithBlock},
    rpc::{
        client::{NoParams, RpcCall},
        json_rpc::{Id, Request},
        types::{Block, BlockNumberOrTag, FilterChanges},
    },
    transports::TransportResult,
};
use tokio::sync::RwLock;
use tracing::debug;

#[derive(Clone)]
pub struct ArchiveHistoryProvider<P> {
    provider: P,
    current_block: Arc<AtomicU64>,
    new_block_filter: Arc<RwLock<HashMap<U256, u64>>>,
}

impl<P> ArchiveHistoryProvider<P> where P: Provider<Ethereum> + Send + Sync + Clone + 'static {}

impl<P> ArchiveHistoryProvider<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn block_number(&self) -> u64 {
        self.current_block.load(Ordering::Relaxed)
    }

    pub fn block_id(&self) -> BlockId {
        BlockId::Number(BlockNumberOrTag::Number(self.block_number()))
    }
}

#[allow(dead_code)]
impl<P> ArchiveHistoryProvider<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(provider: P, start_block: u64) -> Self {
        Self { provider, current_block: Arc::new(AtomicU64::new(start_block)), new_block_filter: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub fn next_block(&self) -> u64 {
        let previous_block = self.current_block.load(Ordering::Relaxed);
        let block = self.current_block.fetch_add(1, Ordering::Relaxed);
        println!("Change block {previous_block} -> {block} ");
        block
    }
}

#[async_trait::async_trait]
impl<P> Provider<Ethereum> for ArchiveHistoryProvider<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    fn root(&self) -> &RootProvider<Ethereum> {
        self.provider.root()
    }

    #[allow(clippy::type_complexity)]
    fn get_block_number(&self) -> ProviderCall<NoParams, U64, BlockNumber> {
        let provider_call = ProviderCall::RpcCall(
            RpcCall::new(Request::new("get_block_number", Id::None, [(); 0]), self.provider.client().transport().clone())
                .map_resp(convert_u64 as fn(U64) -> u64),
        );
        provider_call
    }

    fn call<'req>(&self, tx: &'req <Ethereum as Network>::TransactionRequest) -> EthCall<'req, Ethereum, Bytes> {
        let call = EthCall::new(self.weak_client(), "eth_call", tx).block(self.block_id());
        debug!("call {:?}", self.block_id());
        call
    }

    fn get_storage_at(&self, address: Address, key: U256) -> RpcWithBlock<(Address, U256), StorageValue> {
        debug!("get_storage_at {:?}", self.block_id());
        let rpc_call = RpcWithBlock::from(self.provider.client().request("eth_getStorageAt", (address, key)));
        rpc_call.block_id(self.block_id())
    }

    fn get_block_by_number<'life0, 'async_trait>(
        &'life0 self,
        number: BlockNumberOrTag,
        tx_kind: BlockTransactionsKind,
    ) -> Pin<Box<dyn Future<Output = TransportResult<Option<Block>>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.provider.get_block_by_number(number, tx_kind)
    }

    async fn get_filter_changes<R: RpcRecv>(&self, id: U256) -> TransportResult<Vec<R>> {
        println!("get_filter_changes");

        self.provider.get_filter_changes(id).await
    }

    async fn get_filter_changes_dyn(&self, id: U256) -> TransportResult<FilterChanges> {
        println!("get_filter_changes_dyn");

        self.provider.get_filter_changes_dyn(id).await
    }

    async fn new_block_filter(&self) -> TransportResult<U256> {
        let result = self.provider.new_block_filter().await;
        let cur_block = self.block_number();
        if let Ok(filter_id) = &result {
            self.new_block_filter.write().await.insert(*filter_id, cur_block);
        }
        result
    }
}

#[cfg(test)]
mod test {}
