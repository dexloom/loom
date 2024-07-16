use std::collections::HashMap;

use alloy_primitives::{Address, U256};

#[derive(Debug, Clone, Default)]
pub struct AccountNonceAndBalances {
    nonce: u64,
    balance: HashMap<Address, U256>,
}

impl AccountNonceAndBalances {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_nonce(&self) -> u64 {
        self.nonce
    }

    pub fn set_nonce(&mut self, nonce: u64) -> &mut Self {
        self.nonce = nonce;
        self
    }

    pub fn set_balance(&mut self, token: Address, balance: U256) -> &mut Self {
        let entry = self.balance.entry(token).or_default();
        *entry = balance;
        self
    }

    pub fn add_balance(&mut self, token: Address, balance: U256) -> &mut Self {
        let entry = self.balance.entry(token).or_default();
        *entry += balance;
        self
    }

    pub fn sub_balance(&mut self, token: Address, balance: U256) -> &mut Self {
        let entry = self.balance.entry(token).or_default();
        *entry -= balance;
        self
    }

    pub fn get_eth_balance(&self) -> U256 {
        self.balance.get(&Address::ZERO).cloned().unwrap_or_default()
    }
    pub fn get_balance(&self, token_address: &Address) -> U256 {
        self.balance.get(token_address).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct AccountNonceAndBalanceState {
    accounts: HashMap<Address, AccountNonceAndBalances>,
}

impl AccountNonceAndBalanceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_account(&mut self, account: Address) -> &mut AccountNonceAndBalances {
        self.accounts.entry(account).or_default()
    }

    pub fn get_account(&self, account: &Address) -> Option<&AccountNonceAndBalances> {
        self.accounts.get(account)
    }

    pub fn get_mut_account(&mut self, account: &Address) -> Option<&mut AccountNonceAndBalances> {
        self.accounts.get_mut(account)
    }

    pub fn get_accounts_vec(&self) -> Vec<Address> {
        self.accounts.keys().copied().collect()
    }

    pub fn is_monitored(&self, account: &Address) -> bool {
        self.accounts.contains_key(account)
    }
}
