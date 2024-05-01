use std::collections::HashMap;

use alloy_primitives::{Address, U256};

pub struct AccountNonceAndBalances {
    nonce: u64,
    balance: HashMap<Address, U256>,
}

impl AccountNonceAndBalances {
    pub fn new() -> AccountNonceAndBalances {
        AccountNonceAndBalances {
            nonce: 0,
            balance: HashMap::new(),
        }
    }

    pub fn get_nonce(&self) -> u64 {
        return self.nonce;
    }

    pub fn set_nonce(&mut self, nonce: u64) -> &mut Self {
        self.nonce = nonce;
        self
    }

    pub fn set_balance(&mut self, token: Address, balance: U256) -> &mut Self {
        let mut entry = self.balance.entry(token).or_default();
        *entry = balance;
        self
    }

    pub fn add_balance(&mut self, token: Address, balance: U256) -> &mut Self {
        let mut entry = self.balance.entry(token).or_default();
        *entry += balance;
        self
    }

    pub fn sub_balance(&mut self, token: Address, balance: U256) -> &mut Self {
        let mut entry = self.balance.entry(token).or_default();
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

pub struct AccountNonceAndBalanceState {
    accounts: HashMap<Address, AccountNonceAndBalances>,
}

impl AccountNonceAndBalanceState {
    pub fn new() -> AccountNonceAndBalanceState {
        AccountNonceAndBalanceState {
            accounts: HashMap::new()
        }
    }

    pub fn add_account(&mut self, account: Address) -> &mut AccountNonceAndBalances {
        self.accounts.entry(account).or_insert(AccountNonceAndBalances::new())
    }

    pub fn get_account(&self, account: &Address) -> Option<&AccountNonceAndBalances> {
        self.accounts.get(account)
    }

    pub fn get_mut_account(&mut self, account: &Address) -> Option<&mut AccountNonceAndBalances> {
        self.accounts.get_mut(account)
    }

    pub fn get_accounts_vec(&self) -> Vec<Address> {
        self.accounts.iter().map(|(address, _)| address.clone()).collect()
    }


    pub fn is_monitored(&self, account: &Address) -> bool {
        self.accounts.get(account).is_some()
    }
}