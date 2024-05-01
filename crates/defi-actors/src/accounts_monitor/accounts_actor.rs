use std::convert::Infallible;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use alloy_rpc_types::BlockTransactions;
use async_trait::async_trait;
use log::debug;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;
use tokio::time::sleep;

use defi_entities::{AccountNonceAndBalanceState, BlockHistory};
use defi_events::MarketEvents;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

pub async fn nonce_and_balance_fetcher_worker<P>(
    client: P,
    accounts_state: SharedState<AccountNonceAndBalanceState>,
) -> WorkerResult
    where
        P: Provider + Send + Sync + Clone + 'static
{
    let eth_addr = Address::ZERO;

    loop {
        let accounts = accounts_state.read().await.get_accounts_vec();
        for addr in accounts.into_iter() {
            let nonce = client.get_transaction_count(addr, BlockId::Number(BlockNumberOrTag::Latest)).await;
            let balance = client.get_balance(addr, BlockId::Number(BlockNumberOrTag::Latest)).await;

            match accounts_state.write().await.get_mut_account(&addr) {
                Some(acc) => {
                    if let Ok(nonce) = nonce {
                        acc.set_nonce(nonce);
                    }
                    if let Ok(balance) = balance {
                        acc.set_balance(eth_addr, balance);
                    }
                }
                _ => {}
            };
            debug!("Account {} nonce {:?} balance {:?}", addr, nonce, balance);
        }

        sleep(Duration::from_secs(20)).await;
    }
}

pub async fn nonce_and_balance_monitor_worker(
    accounts_state: SharedState<AccountNonceAndBalanceState>,
    block_history_state: SharedState<BlockHistory>,
    mut market_events: Receiver<MarketEvents>) -> WorkerResult
{
    loop {
        tokio::select! {
            msg = market_events.recv() => {
                let market_event_msg : Result<MarketEvents, RecvError> = msg;
                match market_event_msg {
                    Ok(market_event) => {
                        match market_event {
                            MarketEvents::BlockTxUpdate{ block_number, block_hash } => {
                                if let Some(block_entry) = block_history_state.read().await.get_market_history_entry(&block_hash).cloned() {
                                    if let Some(block) = block_entry.block {
                                        if let BlockTransactions::Full(txs) = block.transactions {
                                            for tx in txs {
                                                let tx_from : Address = tx.from;
                                                if accounts_state.read().await.is_monitored(&tx_from) {
                                                    match accounts_state.write().await.get_mut_account(&tx_from) {
                                                        Some(&mut ref mut account) => {
                                                            let spent = (tx.max_fee_per_gas.unwrap() + tx.max_priority_fee_per_gas.unwrap()) * tx.gas + tx.value.to::<u128>();
                                                            account.sub_balance(Address::ZERO, U256::from(spent));
                                                            account.set_nonce(tx.nonce);
                                                        }
                                                        None=>{}
                                                    }
                                                }

                                                if let Some(to )  = tx.to {
                                                    if accounts_state.read().await.is_monitored(&to) {
                                                        match accounts_state.write().await.get_mut_account(&to) {
                                                            Some(&mut ref mut account) => {
                                                                account.add_balance(Address::ZERO, tx.value);
                                                            }
                                                            None=>{}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                            }
                            _=>{}
                        }
                    }
                    _=>{}
                }
            }
        }
    }
}

#[derive(Accessor, Consumer)]
pub struct NonceAndBalanceMonitorActor<P>
{
    client: P,
    #[accessor]
    accounts_state: Option<SharedState<AccountNonceAndBalanceState>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    market_events: Option<Broadcaster<MarketEvents>>,
}

impl<P> NonceAndBalanceMonitorActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> NonceAndBalanceMonitorActor<P> {
        NonceAndBalanceMonitorActor {
            client,
            accounts_state: None,
            block_history: None,
            market_events: None,
        }
    }
}

#[async_trait]
impl<P> Actor for NonceAndBalanceMonitorActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    async fn start(&mut self) -> ActorResult {
        let monitor_task = tokio::task::spawn(
            nonce_and_balance_monitor_worker(
                self.accounts_state.clone().unwrap(),
                self.block_history.clone().unwrap(),
                self.market_events.clone().unwrap().subscribe().await,
            )
        );

        let fetcher_task = tokio::task::spawn(
            nonce_and_balance_fetcher_worker(
                self.client.clone(),
                self.accounts_state.clone().unwrap(),
            )
        );

        Ok(vec![monitor_task, fetcher_task])
    }
}