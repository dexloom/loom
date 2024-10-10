use alloy_consensus::constants::{EIP1559_TX_TYPE_ID, EIP2930_TX_TYPE_ID, EIP4844_TX_TYPE_ID, LEGACY_TX_TYPE_ID};
use alloy_eips::BlockNumberOrTag;
use alloy_network::{Network, TransactionBuilder};
use alloy_primitives::{Address, BlockNumber, TxHash, U256};
use alloy_provider::Provider;
use alloy_rpc_types::state::StateOverride;
use alloy_rpc_types::{BlockOverrides, TransactionRequest};
use alloy_rpc_types_trace::geth::GethDebugTracingCallOptions;
use alloy_transport::Transport;
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use revm::primitives::bitvec::macros::internal::funty::Fundamental;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::required_state::accounts_vec_len;
use defi_entities::{LatestBlock, Market, MarketState};
use defi_events::{MarketEvents, MempoolEvents, StateUpdateEvent};
use defi_types::{debug_trace_call_diff, GethStateUpdateVec, Mempool, TRACING_CALL_OPTS};
use loom_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

use super::affected_pools::get_affected_pools;
use super::affected_pools_code::{get_affected_pools_from_code, is_pool_code};

lazy_static! {
    static ref COINBASE: Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

/// Process a pending tx from the mempool
#[allow(clippy::too_many_arguments)]
pub async fn pending_tx_state_change_task<P, T, N>(
    client: P,
    tx_hash: TxHash,
    market: SharedState<Market>,
    mempool: SharedState<Mempool>,
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState>,
    affecting_tx: Arc<RwLock<HashMap<TxHash, bool>>>,
    cur_block_number: BlockNumber,
    cur_block_time: u64,
    cur_next_base_fee: u64,
    cur_state_override: StateOverride,
    state_updates_broadcaster: Broadcaster<StateUpdateEvent>,
) -> Result<()>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let mut state_update_vec: GethStateUpdateVec = Vec::new();
    let mut state_required_vec: GethStateUpdateVec = Vec::new();

    let mut merged_state_update_vec: GethStateUpdateVec = Vec::new();

    let mempool_tx = match mempool.read().await.get_tx_by_hash(&tx_hash).cloned() {
        Some(tx) => tx,
        None => return Err(eyre!("MEMPOOL_TX_NOT_FOUND")),
    };

    let tx = match mempool_tx.tx.clone() {
        Some(tx) => tx,
        None => return Err(eyre!("NO_TX_IN_MEMPOOL")),
    };

    let source = mempool_tx.source.clone();

    let mut transaction_request: TransactionRequest = tx.clone().into_request();

    let transaction_type = transaction_request.transaction_type.unwrap_or_default();
    if transaction_type == LEGACY_TX_TYPE_ID || transaction_type == EIP2930_TX_TYPE_ID {
        match transaction_request.gas_price {
            Some(g) => {
                if g < cur_next_base_fee as u128 {
                    transaction_request.set_gas_price(cur_next_base_fee as u128);
                }
            }
            None => {
                error!(
                    "No gas price for gas_price={:?}, max_fee_per_gas={:?}, max_priority_fee_per_gas={:?}, hash={:?}",
                    transaction_request.gas_price,
                    transaction_request.max_fee_per_gas,
                    transaction_request.max_priority_fee_per_gas,
                    mempool_tx.tx_hash
                );
                return Err(eyre!("NO_GAS_PRICE"));
            }
        }
    } else if transaction_type == EIP1559_TX_TYPE_ID {
        match transaction_request.max_fee_per_gas {
            Some(g) => {
                if g < cur_next_base_fee as u128 {
                    transaction_request.set_max_fee_per_gas(cur_next_base_fee as u128);
                }
            }
            None => {
                error!(
                    "No base fee for gas_price={:?}, max_fee_per_gas={:?}, max_priority_fee_per_gas={:?}, hash={:?}",
                    transaction_request.gas_price,
                    transaction_request.max_fee_per_gas,
                    transaction_request.max_priority_fee_per_gas,
                    mempool_tx.tx_hash
                );
                return Err(eyre!("NO_BASE_FEE"));
            }
        }
    } else if transaction_type == EIP4844_TX_TYPE_ID {
        // ignore blob tx
        warn!("Ignore EIP4844 transaction: hash={:?}", mempool_tx.tx_hash);
        return Ok(());
    } else {
        warn!("Unknown transaction type: type={}, hash={:?}", transaction_type, mempool_tx.tx_hash);
        return Err(eyre!("UNKNOWN_TX_TYPE"));
    }

    let call_opts: GethDebugTracingCallOptions = GethDebugTracingCallOptions {
        block_overrides: Some(BlockOverrides {
            number: Some(U256::from(cur_block_number)),
            time: Some(cur_block_time),
            coinbase: Some(*COINBASE),
            base_fee: Some(U256::from(cur_next_base_fee)),
            ..Default::default()
        }),
        state_overrides: Some(cur_state_override.clone()),
        ..TRACING_CALL_OPTS.clone()
    };

    if !(*affecting_tx.read().await.get(&tx_hash).unwrap_or(&true)) {
        return Err(eyre!("NON_AFFECTING_TX"));
    }

    let diff_trace_result =
        debug_trace_call_diff(client.clone(), transaction_request, BlockNumberOrTag::Latest.into(), Some(call_opts)).await;
    match diff_trace_result {
        Ok((pre, post)) => {
            state_required_vec.push(pre.clone());
            state_update_vec.push(post.clone());

            merged_state_update_vec.push(pre);
            merged_state_update_vec.push(post);
        }
        Err(e) => {
            mempool.write().await.set_failed(tx.hash);
            error!("debug_trace_call error : {} : {:?}", e, tx.hash);
        }
    }

    let affected_pools = get_affected_pools(market.clone(), &state_update_vec).await;
    match affected_pools {
        Ok(affected_pools) => {
            let storage_len = accounts_vec_len(&state_update_vec);

            debug!("Mempool affected pools {:?} {} update len : {} strg : {}", tx_hash, source, affected_pools.len(), storage_len);

            affecting_tx.write().await.insert(tx_hash, !affected_pools.is_empty());

            //TODO : Fix Latest header is empty
            if let Some(latest_header) = latest_block.read().await.block_header.clone() {
                let next_block_number = latest_header.number.as_u64() + 1;
                let next_block_timestamp = latest_header.timestamp.as_u64() + 12;

                if !affected_pools.is_empty() {
                    let cur_state_db = market_state.read().await.state_db.clone();
                    let request = StateUpdateEvent::new(
                        next_block_number,
                        next_block_timestamp,
                        cur_next_base_fee,
                        cur_state_db,
                        state_update_vec,
                        Some(state_required_vec.clone()),
                        affected_pools,
                        vec![tx_hash],
                        vec![mempool_tx.tx.clone().unwrap()],
                        "pending_tx_searcher".to_string(),
                        9000,
                    );
                    if let Err(e) = state_updates_broadcaster.send(request).await {
                        error!("state_updates_broadcaster : {}", e)
                    }
                }
            } else {
                error!("Latest header is empty")
            }

            if is_pool_code(&merged_state_update_vec) {
                match get_affected_pools_from_code(client, market.clone(), &merged_state_update_vec).await {
                    Ok(affected_pools) => {
                        match affecting_tx.write().await.entry(tx_hash) {
                            Entry::Occupied(mut v) => {
                                if !v.get() {
                                    v.insert(!affected_pools.is_empty());
                                }
                            }
                            Entry::Vacant(v) => {
                                v.insert(!affected_pools.is_empty());
                            }
                        };

                        debug!("Mempool code pools {} {} update len : {}", tx_hash, source, affected_pools.len());

                        if let Some(latest_header) = latest_block.read().await.block_header.clone() {
                            let block_number = latest_header.number.as_u64() + 1;
                            let block_timestamp = latest_header.timestamp.as_u64() + 12;

                            if !affected_pools.is_empty() {
                                let cur_state = market_state.read().await.clone();
                                let request = StateUpdateEvent::new(
                                    block_number,
                                    block_timestamp,
                                    cur_next_base_fee,
                                    cur_state.state_db,
                                    merged_state_update_vec,
                                    None,
                                    affected_pools,
                                    vec![tx_hash],
                                    vec![mempool_tx.tx.unwrap()],
                                    "poolcode_searcher".to_string(),
                                    3000,
                                );
                                if let Err(e) = state_updates_broadcaster.send(request).await {
                                    error!("state_updates_broadcaster : {}", e)
                                }
                            }
                        } else {
                            error!("Latest header is empty")
                        }
                    }
                    Err(e) => {
                        debug!("code affected pools error : {e}")
                    }
                }
            }
            Ok(())
        }
        Err(e) => {
            affecting_tx.write().await.insert(tx_hash, false);
            error!("affected pools error : {}", e);
            Err(eyre!("AFFECTED_POOLS_ERR"))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn pending_tx_state_change_worker<P, T, N>(
    client: P,
    market: SharedState<Market>,
    mempool: SharedState<Mempool>,
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState>,
    mempool_events_rx: Broadcaster<MempoolEvents>,
    market_events_rx: Broadcaster<MarketEvents>,
    state_updates_broadcaster: Broadcaster<StateUpdateEvent>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    subscribe!(mempool_events_rx);
    subscribe!(market_events_rx);

    let affecting_tx: Arc<RwLock<HashMap<TxHash, bool>>> = Arc::new(RwLock::new(HashMap::new()));
    let mut cur_next_base_fee = 0;
    let mut cur_block_number: Option<BlockNumber> = None;
    let mut cur_block_time: Option<u64> = None;
    let mut cur_state_override: StateOverride = StateOverride::default();

    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                if let Ok(msg) = msg {
                    let market_event_msg : MarketEvents = msg;
                    if let MarketEvents::BlockHeaderUpdate{ block_number, block_hash, timestamp, base_fee, next_base_fee } = market_event_msg {
                        debug!("Block header update {} {} base_fee {} ", block_number, block_hash, base_fee);
                        cur_block_number = Some( block_number.as_u64() + 1);
                        cur_block_time = Some(timestamp + 12 );
                        cur_next_base_fee = next_base_fee;

                        for _counter in 0..5  {
                            if let Ok(msg) = market_events_rx.recv().await {
                                if matches!(msg, MarketEvents::BlockStateUpdate{..} ) {
                                    cur_state_override = latest_block.read().await.node_state_override();
                                    debug!("Block state update received {} {}", block_number, block_hash);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            msg = mempool_events_rx.recv() => {
                if let Ok(msg) = msg {
                    let mempool_event_msg : MempoolEvents = msg;
                    if let MempoolEvents::MempoolActualTxUpdate{ tx_hash }  = mempool_event_msg {
                        tokio::task::spawn(
                            pending_tx_state_change_task(
                                client.clone(),
                                tx_hash,
                                market.clone(),
                                mempool.clone(),
                                latest_block.clone(),
                                market_state.clone(),
                                affecting_tx.clone(),
                                cur_block_number.unwrap_or_default(),
                                cur_block_time.unwrap_or_default(),
                                cur_next_base_fee,
                                cur_state_override.clone(),
                                state_updates_broadcaster.clone(),
                            )
                        );
                    }
                }
            }
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct PendingTxStateChangeProcessorActor<P, T, N> {
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
    #[consumer]
    mempool_events_rx: Option<Broadcaster<MempoolEvents>>,
    #[producer]
    state_updates_tx: Option<Broadcaster<StateUpdateEvent>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> PendingTxStateChangeProcessorActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> PendingTxStateChangeProcessorActor<P, T, N> {
        PendingTxStateChangeProcessorActor {
            client,
            market: None,
            mempool: None,
            market_state: None,
            latest_block: None,
            market_events_rx: None,
            mempool_events_rx: None,
            state_updates_tx: None,
            _t: PhantomData,
            _n: PhantomData,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            market: Some(bc.market()),
            mempool: Some(bc.mempool()),
            market_state: Some(bc.market_state()),
            latest_block: Some(bc.latest_block()),
            market_events_rx: Some(bc.market_events_channel()),
            mempool_events_rx: Some(bc.mempool_events_channel()),
            state_updates_tx: Some(bc.state_update_channel()),
            ..self
        }
    }
}

impl<P, T, N> Actor for PendingTxStateChangeProcessorActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(pending_tx_state_change_worker(
            self.client.clone(),
            self.market.clone().unwrap(),
            self.mempool.clone().unwrap(),
            self.latest_block.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.mempool_events_rx.clone().unwrap(),
            self.market_events_rx.clone().unwrap(),
            self.state_updates_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "PendingTxStateChangeProcessorActor"
    }
}
