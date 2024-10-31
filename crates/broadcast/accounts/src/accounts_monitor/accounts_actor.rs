use std::marker::PhantomData;
use std::time::Duration;

use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_network::Network;
use alloy_primitives::{Address, Log, U256};
use alloy_provider::Provider;
use alloy_rpc_types::BlockTransactions;
use alloy_sol_types::SolEventInterface;
use alloy_transport::Transport;
use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer};
use loom_core_blockchain::Blockchain;
use loom_defi_entities::{AccountNonceAndBalanceState, BlockHistory};
use loom_defi_events::MarketEvents;
use loom_protocol_abi::IERC20::IERC20Events;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::sleep;
use tracing::debug;

pub async fn nonce_and_balance_fetcher_worker<P, T, N>(
    client: P,
    accounts_state: SharedState<AccountNonceAndBalanceState>,
    only_once: bool,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    let eth_addr = Address::ZERO;

    loop {
        let accounts = accounts_state.read().await.get_accounts_vec();
        for addr in accounts.into_iter() {
            let nonce = client.get_transaction_count(addr).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await;
            let balance = client.get_balance(addr).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await;

            if let Some(acc) = accounts_state.write().await.get_mut_account(&addr) {
                if let Ok(nonce) = nonce {
                    acc.set_nonce(nonce);
                }
                if let Ok(balance) = balance {
                    acc.set_balance(eth_addr, balance);
                }
            };
            debug!("Account {} nonce {:?} balance {:?}", addr, nonce, balance);
        }
        if only_once {
            break;
        }

        sleep(Duration::from_secs(20)).await;
    }
    Ok("Nonce and balance fetcher finished".to_string())
}

pub async fn nonce_and_balance_monitor_worker(
    accounts_state: SharedState<AccountNonceAndBalanceState>,
    block_history_state: SharedState<BlockHistory>,
    market_events_rx: Broadcaster<MarketEvents>,
) -> WorkerResult {
    let mut market_events = market_events_rx.subscribe().await;

    loop {
        tokio::select! {
            msg = market_events.recv() => {
                let market_event_msg : Result<MarketEvents, RecvError> = msg;
                if let Ok(market_event_msg) = market_event_msg {
                    match market_event_msg {
                        MarketEvents::BlockTxUpdate{ block_hash, .. }=>{
                            if let Some(block_entry) = block_history_state.read().await.get_entry(&block_hash).cloned() {
                                if let Some(block) = block_entry.block {
                                    if let BlockTransactions::Full(txs) = block.transactions {

                                        // acquire accounts shared state write lock
                                        let mut accounts_lock = accounts_state.write().await;

                                        for tx in txs {
                                            let tx_from : Address = tx.from;
                                            if accounts_lock.is_monitored(&tx_from) {
                                                if let Some(&mut ref mut account) = accounts_lock.get_mut_account(&tx_from) {
                                                    let spent = (tx.max_fee_per_gas.unwrap() + tx.max_priority_fee_per_gas.unwrap()) * tx.gas as u128 + tx.value.to::<u128>();
                                                    let value = U256::from(spent);
                                                    account.sub_balance(Address::ZERO, value).set_nonce(tx.nonce+1);
                                                    debug!("Account {} : sub ETH balance {} -> {} nonce {}", tx_from, value, account.get_eth_balance(), tx.nonce+1);
                                                }
                                            }

                                            if let Some(to )  = tx.to {
                                                if accounts_lock.is_monitored(&to) {
                                                    if let Some(&mut ref mut account) = accounts_lock.get_mut_account(&to) {
                                                        account.add_balance(Address::ZERO, tx.value);
                                                        debug!("Account {} : add ETH balance {} -> {}", to, tx.value, account.get_eth_balance());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        MarketEvents::BlockLogsUpdate { block_hash, .. }=>{
                            if let Some(block_entry) = block_history_state.read().await.get_entry(&block_hash) {
                                if let Some(logs) = &block_entry.logs {

                                    // acquire accounts shared state write lock
                                    let mut accounts_lock = accounts_state.write().await;

                                    for log_entry in logs.iter() {
                                        let log_entry: Option<Log> = Log::new(log_entry.address(), log_entry.topics().to_vec(), log_entry.data().data.clone());
                                        if let Some(log_entry) = log_entry {
                                            if let Ok(event) = IERC20Events::decode_log(&log_entry, false ){
                                                if let  IERC20Events::Transfer(event) = event.data {
                                                    //debug!("ERC20TransferEvent {} : {:?}", log_entry.address, event);
                                                    if accounts_lock.is_monitored(&event.to) {
                                                        if let Some(&mut ref mut account) = accounts_lock.get_mut_account(&event.to) {
                                                            account.add_balance(log_entry.address, event.value);
                                                            debug!("Account {} : add ERC20 {} balance {} -> {}", event.to, log_entry.address, event.value, account.get_balance(&log_entry.address));
                                                        }
                                                    } else if accounts_lock.is_monitored(&event.from) {
                                                        if let Some(&mut ref mut account) = accounts_lock.get_mut_account(&event.from) {
                                                            account.sub_balance(log_entry.address, event.value);
                                                            debug!("Account {} : sub ERC20 {} balance {} -> {}", event.from, log_entry.address, event.value, account.get_balance(&log_entry.address));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    drop(accounts_lock);
                                }
                            }
                        }
                        _=>{}
                    }
                }
            }
        }
    }
}

#[derive(Accessor, Consumer)]
pub struct NonceAndBalanceMonitorActor<P, T, N> {
    client: P,
    only_once: bool,
    with_fetcher: bool,
    #[accessor]
    accounts_nonce_and_balance: Option<SharedState<AccountNonceAndBalanceState>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    market_events: Option<Broadcaster<MarketEvents>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> NonceAndBalanceMonitorActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> NonceAndBalanceMonitorActor<P, T, N> {
        NonceAndBalanceMonitorActor {
            client,
            accounts_nonce_and_balance: None,
            block_history: None,
            market_events: None,
            only_once: false,
            with_fetcher: true,
            _t: PhantomData,
            _n: PhantomData,
        }
    }

    pub fn only_once(self) -> Self {
        Self { only_once: true, ..self }
    }

    pub fn without_fetcher(self) -> Self {
        Self { with_fetcher: false, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain) -> NonceAndBalanceMonitorActor<P, T, N> {
        NonceAndBalanceMonitorActor {
            accounts_nonce_and_balance: Some(bc.nonce_and_balance()),
            block_history: Some(bc.block_history().clone()),
            market_events: Some(bc.market_events_channel().clone()),
            ..self
        }
    }
}

impl<P, T, N> Actor for NonceAndBalanceMonitorActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let mut handles = Vec::new();

        if self.with_fetcher {
            let fetcher_task = tokio::task::spawn(nonce_and_balance_fetcher_worker(
                self.client.clone(),
                self.accounts_nonce_and_balance.clone().unwrap(),
                self.only_once,
            ));

            if self.only_once {
                loop {
                    if fetcher_task.is_finished() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            } else {
                handles.push(fetcher_task);
            }
        }

        let monitor_task = tokio::task::spawn(nonce_and_balance_monitor_worker(
            self.accounts_nonce_and_balance.clone().unwrap(),
            self.block_history.clone().unwrap(),
            self.market_events.clone().unwrap(),
        ));
        handles.push(monitor_task);

        Ok(handles)
    }

    fn name(&self) -> &'static str {
        "NonceAndBalanceMonitorActor"
    }
}
