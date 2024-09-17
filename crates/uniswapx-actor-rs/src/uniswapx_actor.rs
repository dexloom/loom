use std::marker::PhantomData;

use crate::shared_state::uniswapx_orders::UniswapXOrders;
use alloy_network::Network;
use alloy_primitives::{address, Address};
use alloy_provider::Provider;
use alloy_rpc_types_eth::{Filter, Header};
use alloy_sol_types::SolEvent;
use alloy_transport::Transport;
use async_trait::async_trait;
use defi_abi::uniswapx::exclusive_dutch_order_reactor::ReactorEvents;
use defi_blockchain::Blockchain;
use defi_events::{IntentEvent, MessageBlockHeader};
use eyre::eyre;
use lazy_static::lazy_static;
use log::{error, info};
use loom_actors::{run_async, subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use uniswapx_client_rs::client::{UniswapXApiClient, UniswapXApiConfig};
use uniswapx_client_rs::types::order::OrderStatus;
use uniswapx_client_rs::types::OrdersQueryBuilder;
use uniswapx_client_rs::types::SortKey::CreatedAt;

lazy_static! {
    pub static ref UNISWAPX_REACTOR_ADDRESS: Address = address!("6000da47483062A0D734Ba3dc7576Ce6A0B645C4");
}

pub async fn uniswapx_orders_fetcher_worker<P, T, N>(
    _client: P,
    uniswapx_orders: SharedState<UniswapXOrders>,
    intent_events_tx: Broadcaster<IntentEvent>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    let uniswapx_client = UniswapXApiClient::new(UniswapXApiConfig::default());
    // https://api.uniswap.org/v2/orders?limit=100&chainId=1&sortKey=createdAt&desc=true&sort=lt(90000000000)
    let query = OrdersQueryBuilder::default()
        .limit(100u32)
        .chain_id(1)
        .sort_key(CreatedAt)
        .desc(true)
        .sort("lt(90000000000)".to_string())
        .build()?;
    loop {
        let result = uniswapx_client.orders(&query).await;
        match result {
            Ok(orders_resp) => {
                let mut uniswapx_orders_guard = uniswapx_orders.write().await;
                for order in orders_resp.orders {
                    match order.order_status {
                        OrderStatus::Open => {
                            let previous = uniswapx_orders_guard.open_orders.insert(order.order_hash, order.clone());
                            if previous.is_none() {
                                info!("New order: {:?}", &order);
                                let event = IntentEvent::from(order.clone());
                                run_async!(intent_events_tx.send(event));
                            }
                        }
                        _ => {
                            let removed = uniswapx_orders_guard.open_orders.remove(&order.order_hash);
                            if removed.is_some() {
                                info!("Removed order: {:?}", order);
                            }
                        }
                    }
                }
                drop(uniswapx_orders_guard);
            }
            Err(e) => {
                error!("UniswapX orders fetch error: {:?}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

pub async fn uniswapx_filled_orders_worker<P, T, N>(
    client: P,
    uniswapx_orders: SharedState<UniswapXOrders>,
    intent_events_tx: Broadcaster<IntentEvent>,
    block_header_rx: Broadcaster<MessageBlockHeader>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    subscribe!(block_header_rx);
    loop {
        let block_header = match block_header_rx.recv().await {
            Ok(message_block_header) => message_block_header.inner,
            Err(e) => match e {
                RecvError::Closed => {
                    error!("Block header channel closed");
                    break Err(eyre!("BLOCK_HEADER_RX_CLOSED"));
                }
                RecvError::Lagged(lag) => {
                    error!("Block header channel lagged by {} messages", lag);
                    continue;
                }
            },
        };

        let filter = Filter::new()
            .at_block_hash(block_header.header.hash)
            .address(*UNISWAPX_REACTOR_ADDRESS)
            .event_signature(ReactorEvents::Fill::SIGNATURE_HASH);

        let logs = client.get_logs(&filter).await?;
        for log in logs {
            let fill_event = match ReactorEvents::Fill::decode_log(&log.inner, true) {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to decode fill event: {:?}", e);
                    continue;
                }
            };
            let mut uniswapx_orders_guard = uniswapx_orders.write().await;

            let order = uniswapx_orders_guard.open_orders.remove(&fill_event.orderHash);
            if order.is_some() {
                info!("Order filled: {:?}", order);
            }
            drop(uniswapx_orders_guard);
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct UniswapXFetchOrdersActor<P, T, N> {
    client: P,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
    #[accessor]
    uniswapx_orders: Option<SharedState<UniswapXOrders>>,
    #[producer]
    intent_events_tx: Option<Broadcaster<IntentEvent>>,
    #[consumer]
    block_header_rx: Option<Broadcaster<MessageBlockHeader>>,
}

impl<P, T, N> UniswapXFetchOrdersActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> UniswapXFetchOrdersActor<P, T, N> {
        UniswapXFetchOrdersActor {
            client,
            _t: PhantomData,
            _n: PhantomData,
            uniswapx_orders: None,
            intent_events_tx: None,
            block_header_rx: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { block_header_rx: Some(bc.new_block_headers_channel()), ..self }
    }
}

impl<P, T, N> Actor for UniswapXFetchOrdersActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&mut self) -> ActorResult {
        let fetcher_task = tokio::task::spawn(uniswapx_orders_fetcher_worker(
            self.client.clone(),
            self.uniswapx_orders.clone().unwrap(),
            self.intent_events_tx.clone().unwrap(),
        ));

        let filled_orders_task = tokio::task::spawn(uniswapx_filled_orders_worker(
            self.client.clone(),
            self.uniswapx_orders.clone().unwrap(),
            self.intent_events_tx.clone().unwrap(),
            self.block_header_rx.clone().unwrap(),
        ));

        Ok(vec![fetcher_task, filled_orders_task])
    }

    fn name(&self) -> &'static str {
        "UniswapXFetchOrdersActor"
    }
}
