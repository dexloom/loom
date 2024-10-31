use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::trace::geth::AccountState;
use loom_types_blockchain::GethStateUpdate;

pub fn account_state_with_nonce_and_balance(nonce: u64, balance: U256) -> AccountState {
    AccountState { balance: Some(balance), code: None, nonce: Some(nonce), storage: Default::default() }
}
pub fn account_state_add_storage(account_state: AccountState, key: B256, value: B256) -> AccountState {
    let mut account_state = account_state;
    account_state.storage.insert(key, value);
    account_state
}

pub fn geth_state_update_add_account(update: GethStateUpdate, address: Address, account_state: AccountState) -> GethStateUpdate {
    let mut update = update;
    update.insert(address, account_state);
    update
}
