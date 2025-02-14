use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::marker::PhantomData;

use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_network::Network;
use alloy_primitives::{Address, Bytes, U256};
use alloy_provider::Provider;
use alloy_rpc_types_trace::geth::AccountState;
use eyre::{eyre, Result};
use loom_core_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_core_actors_macros::Accessor;
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_defi_address_book::TokenAddressEth;
use loom_evm_utils::{BalanceCheater, NWETH};
use loom_types_blockchain::GethStateUpdate;
use loom_types_entities::{AccountNonceAndBalanceState, MarketState, TxSigners};
use revm::{Database, DatabaseCommit, DatabaseRef};
use tracing::{debug, error, trace};

async fn fetch_account_state<P, N>(client: P, address: Address) -> Result<AccountState>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    let code = client.get_code_at(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.ok();
    let balance = client.get_balance(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.ok();
    let nonce = client.get_transaction_count(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.ok();

    Ok(AccountState { balance, code, nonce, storage: BTreeMap::new() })
}

async fn set_monitor_token_balance(
    account_nonce_balance_state: Option<SharedState<AccountNonceAndBalanceState>>,
    owner: Address,
    token: Address,
    balance: U256,
) {
    if let Some(account_nonce_balance) = account_nonce_balance_state {
        debug!("set_monitor_balance {} {} {}", owner, token, balance);
        let mut account_nonce_balance_guard = account_nonce_balance.write().await;
        let entry = account_nonce_balance_guard.get_entry_or_default(owner);
        debug!("set_monitor_balance {:?}", entry);

        entry.add_balance(token, balance);
    }
}

async fn set_monitor_nonce(account_nonce_balance_state: Option<SharedState<AccountNonceAndBalanceState>>, owner: Address, nonce: u64) {
    if let Some(account_nonce_balance) = account_nonce_balance_state {
        debug!("set_monitor_nonce {} {}", owner, nonce);
        let mut account_nonce_balance_guard = account_nonce_balance.write().await;
        let entry = account_nonce_balance_guard.get_entry_or_default(owner);
        debug!("set_monitor_nonce {:?}", entry);
        entry.set_nonce(nonce);
    }
}

pub async fn preload_market_state<P, N, DB>(
    client: P,
    copied_accounts_vec: Vec<Address>,
    new_accounts_vec: Vec<(Address, u64, U256, Option<Bytes>)>,
    token_balances_vec: Vec<(Address, Address, U256)>,
    market_state: SharedState<MarketState<DB>>,
    account_nonce_balance_state: Option<SharedState<AccountNonceAndBalanceState>>,
) -> WorkerResult
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Database + DatabaseCommit + Send + Sync + Clone + 'static,
{
    let mut market_state_guard = market_state.write().await;

    let mut state: GethStateUpdate = BTreeMap::new();

    for address in copied_accounts_vec {
        trace!("Loading address : {address}");
        let acc_state = fetch_account_state(client.clone(), address).await?;

        set_monitor_token_balance(
            account_nonce_balance_state.clone(),
            address,
            NWETH::NATIVE_ADDRESS,
            acc_state.balance.unwrap_or_default(),
        )
        .await;

        set_monitor_nonce(account_nonce_balance_state.clone(), address, acc_state.nonce.unwrap_or_default()).await;
        trace!("Loaded address : {address} {:?}", acc_state);

        state.insert(address, acc_state);
    }

    for (address, nonce, balance, code) in new_accounts_vec {
        trace!("new_accounts added {} {} {}", address, nonce, balance);
        set_monitor_token_balance(account_nonce_balance_state.clone(), address, NWETH::NATIVE_ADDRESS, balance).await;
        state.insert(address, AccountState { balance: Some(balance), code, nonce: Some(nonce), storage: BTreeMap::new() });
    }

    for (token, owner, balance) in token_balances_vec {
        if token == TokenAddressEth::ETH_NATIVE {
            match state.entry(owner) {
                Entry::Vacant(e) => {
                    e.insert(AccountState { balance: Some(balance), nonce: Some(0), code: None, storage: BTreeMap::new() });
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().balance = Some(balance);
                }
            }
        } else {
            match state.entry(token) {
                Entry::Vacant(e) => {
                    let mut acc_state = fetch_account_state(client.clone(), token).await?;
                    acc_state.storage.insert(BalanceCheater::get_balance_cell(token, owner)?.into(), balance.into());
                    e.insert(acc_state);
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().storage.insert(BalanceCheater::get_balance_cell(token, owner)?.into(), balance.into());
                }
            }
        }

        set_monitor_token_balance(account_nonce_balance_state.clone(), owner, token, balance).await;
    }
    market_state_guard.apply_geth_update(state);

    Ok("DONE".to_string())
}

#[allow(dead_code)]
#[derive(Accessor)]
pub struct MarketStatePreloadedOneShotActor<P, N, DB> {
    name: &'static str,
    client: P,
    copied_accounts: Vec<Address>,
    new_accounts: Vec<(Address, u64, U256, Option<Bytes>)>,
    token_balances: Vec<(Address, Address, U256)>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[accessor]
    account_nonce_balance_state: Option<SharedState<AccountNonceAndBalanceState>>,
    _n: PhantomData<N>,
}

#[allow(dead_code)]
impl<P, N, DB> MarketStatePreloadedOneShotActor<P, N, DB>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    fn name(&self) -> &'static str {
        self.name
    }

    pub fn new(client: P) -> Self {
        Self {
            name: "MarketStatePreloadedOneShotActor",
            client,
            copied_accounts: Vec::new(),
            new_accounts: Vec::new(),
            token_balances: Vec::new(),
            market_state: None,
            account_nonce_balance_state: None,
            _n: PhantomData,
        }
    }

    pub fn with_name(self, name: &'static str) -> Self {
        Self { name, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain, state: &BlockchainState<DB>) -> Self {
        Self { account_nonce_balance_state: Some(bc.nonce_and_balance()), market_state: Some(state.market_state_commit()), ..self }
    }

    pub fn with_signers(self, tx_signers: SharedState<TxSigners>) -> Self {
        match tx_signers.try_read() {
            Ok(signers) => {
                let mut addresses = self.copied_accounts;
                addresses.extend(signers.get_address_vec());
                Self { copied_accounts: addresses, ..self }
            }
            Err(e) => {
                error!("tx_signers.try_read() {}", e);
                self
            }
        }
    }

    pub fn with_copied_account(self, address: Address) -> Self {
        let mut copied_accounts = self.copied_accounts;
        copied_accounts.push(address);
        Self { copied_accounts, ..self }
    }

    pub fn with_copied_accounts(self, address_vec: Vec<Address>) -> Self {
        let mut copied_accounts = self.copied_accounts;
        copied_accounts.extend(address_vec);
        Self { copied_accounts, ..self }
    }

    pub fn with_new_account(self, address: Address, nonce: u64, balance: U256, code: Option<Bytes>) -> Self {
        let mut new_accounts = self.new_accounts;
        new_accounts.push((address, nonce, balance, code));
        Self { new_accounts, ..self }
    }

    pub fn with_token_balance(self, token: Address, owner: Address, balance: U256) -> Self {
        let mut token_balances = self.token_balances;
        token_balances.push((token, owner, balance));
        Self { token_balances, ..self }
    }
}

impl<P, N, DB> Actor for MarketStatePreloadedOneShotActor<P, N, DB>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Database + DatabaseCommit + Send + Sync + Clone + 'static,
{
    fn start_and_wait(&self) -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?; // we need a different runtime to wait for the result
        let handler = rt.spawn(preload_market_state(
            self.client.clone(),
            self.copied_accounts.clone(),
            self.new_accounts.clone(),
            self.token_balances.clone(),
            self.market_state.clone().unwrap(),
            self.account_nonce_balance_state.clone(),
        ));

        self.wait(Ok(vec![handler]))?;
        rt.shutdown_background();
        Ok(())
    }

    fn start(&self) -> ActorResult {
        Err(eyre!("NEED_TO_BE_WAITED"))
    }

    fn name(&self) -> &'static str {
        "MarketStatePreloadedOneShotActor"
    }
}
