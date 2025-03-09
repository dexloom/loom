use std::collections::HashMap;

use crate::EntityAddress;
use alloy_primitives::{Address, U256};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};

#[derive(Debug, Clone, Default)]
pub struct AccountNonceAndBalances {
    nonce: u64,
    balance: HashMap<EntityAddress, U256>,
}

impl AccountNonceAndBalances {
    pub fn new() -> Self {
        Self { nonce: 0, balance: HashMap::new() }
    }

    pub fn get_nonce(&self) -> u64 {
        self.nonce
    }

    pub fn set_nonce(&mut self, nonce: u64) -> &mut Self {
        self.nonce = nonce;
        self
    }

    pub fn set_balance(&mut self, token: EntityAddress, balance: U256) -> &mut Self {
        let entry = self.balance.entry(token).or_default();
        *entry = balance;
        self
    }

    pub fn add_balance(&mut self, token: EntityAddress, balance: U256) -> &mut Self {
        let entry = self.balance.entry(token).or_default();
        if let Some(value) = entry.checked_add(balance) {
            *entry = value
        }
        self
    }

    pub fn sub_balance(&mut self, token: EntityAddress, balance: U256) -> &mut Self {
        let entry = self.balance.entry(token).or_default();
        if let Some(value) = entry.checked_sub(balance) {
            *entry = value
        }
        self
    }

    pub fn get_eth_balance(&self) -> U256 {
        self.balance.get(&EntityAddress::default()).cloned().unwrap_or_default()
    }
    pub fn get_balance(&self, token_address: &EntityAddress) -> U256 {
        self.balance.get(token_address).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct AccountNonceAndBalanceState {
    accounts: HashMap<EntityAddress, AccountNonceAndBalances>,
}

impl AccountNonceAndBalanceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_account(&mut self, account: EntityAddress) -> &mut AccountNonceAndBalances {
        self.accounts.entry(account).or_default()
    }

    pub fn get_account(&self, account: &EntityAddress) -> Option<&AccountNonceAndBalances> {
        self.accounts.get(account)
    }

    pub fn get_mut_account(&mut self, account: &EntityAddress) -> Option<&mut AccountNonceAndBalances> {
        self.accounts.get_mut(account)
    }

    pub fn get_accounts_vec(&self) -> Vec<EntityAddress> {
        self.accounts.keys().copied().collect()
    }

    pub fn is_monitored(&self, account: &EntityAddress) -> bool {
        self.accounts.contains_key(account)
    }

    pub fn get_entry_or_default(&mut self, account: EntityAddress) -> &mut AccountNonceAndBalances {
        self.accounts.entry(account).or_default()
    }
}
