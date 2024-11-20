use crate::loom_data_types::{LoomDataTypes, LoomDataTypesEthereum};
use crate::{AccountNonceAndTransactions, FetchState, GethStateUpdate, MempoolTx};
use alloy_consensus::Transaction as TransactionTrait;
use alloy_primitives::{Address, BlockNumber, TxHash};
use alloy_provider::network::TransactionResponse;
use alloy_rpc_types_eth::{Log, Transaction};
use chrono::{DateTime, Utc};
use eyre::{eyre, Result};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Mempool<D: LoomDataTypes = LoomDataTypesEthereum> {
    pub txs: HashMap<TxHash, MempoolTx<D>>,
    accounts: HashMap<Address, AccountNonceAndTransactions>,
}

impl Mempool<LoomDataTypesEthereum> {
    pub fn new() -> Mempool<LoomDataTypesEthereum> {
        Mempool { txs: HashMap::new(), accounts: HashMap::new() }
    }

    pub fn len(&self) -> usize {
        self.txs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.txs.is_empty()
    }

    pub fn is_tx(&self, tx_hash: &TxHash) -> bool {
        self.txs.contains_key(tx_hash)
    }

    pub fn add_tx(&mut self, tx: Transaction) -> &mut Self {
        let tx_hash: TxHash = tx.tx_hash();
        let entry = self.txs.entry(tx_hash).or_default();
        entry.tx = Some(tx);
        self
    }

    pub fn add_tx_logs(&mut self, tx_hash: TxHash, logs: Vec<Log>) -> &mut Self {
        let entry = self.txs.entry(tx_hash).or_default();
        entry.logs = Some(logs);
        self
    }

    pub fn add_tx_state_change(&mut self, tx_hash: TxHash, state_update: GethStateUpdate) -> &mut Self {
        let entry = self.txs.entry(tx_hash).or_default();
        entry.state_update = Some(state_update);
        self
    }

    pub fn filter_by_gas_price(&self, gas_price: u128) -> Vec<&MempoolTx<LoomDataTypesEthereum>> {
        self.txs
            .values()
            .filter(|&item| {
                item.mined.is_none() && item.tx.clone().map_or_else(|| false, |i| TransactionTrait::max_fee_per_gas(&i) >= gas_price)
            })
            .collect()
    }

    pub fn filter_ok_by_gas_price(&self, gas_price: u128) -> Vec<&MempoolTx<LoomDataTypesEthereum>> {
        self.txs
            .values()
            .filter(|&item| {
                item.mined.is_none()
                    && !item.failed.unwrap_or(false)
                    && item.tx.clone().map_or_else(|| false, |i| TransactionTrait::max_fee_per_gas(&i) >= gas_price)
            })
            .collect()
    }

    pub fn filter_on_block(&self, block_number: BlockNumber) -> Vec<&MempoolTx<LoomDataTypesEthereum>> {
        self.txs.values().filter(|&item| item.mined == Some(block_number)).collect()
    }

    pub fn is_mined(&self, tx_hash: &TxHash) -> bool {
        match self.txs.get(tx_hash) {
            Some(tx) => tx.mined.is_some(),
            None => false,
        }
    }

    pub fn is_failed(&self, tx_hash: &TxHash) -> bool {
        match self.txs.get(tx_hash) {
            Some(e) => e.failed.unwrap_or(false),
            None => false,
        }
    }

    pub fn clean(&mut self) {
        self.txs = Default::default();
        self.accounts = Default::default();
    }

    pub fn clean_txs(&mut self, max_block_number: BlockNumber, max_time: DateTime<Utc>) {
        self.txs = self
            .txs
            .clone()
            .into_iter()
            .filter(|(_, v)| v.mined.unwrap_or(max_block_number + 1) > max_block_number && v.time > max_time)
            .collect();
    }

    pub fn set_mined(&mut self, tx_hash: TxHash, block_number: BlockNumber) -> &mut Self {
        let entry = self.txs.entry(tx_hash).or_default();
        entry.mined = Some(block_number);
        self
    }

    pub fn set_failed(&mut self, tx_hash: TxHash) {
        if let Entry::Occupied(mut e) = self.txs.entry(tx_hash) {
            let value = e.get_mut();
            value.failed = Some(true)
        }
    }

    pub fn set_nonce(&mut self, account: Address, nonce: u64) -> &mut Self {
        let entry = self.accounts.entry(account).or_default();
        entry.set_nonce(Some(nonce));
        self
    }

    pub fn is_valid_tx(&self, tx: &Transaction) -> bool {
        self.accounts.get(&tx.from).map_or_else(|| true, |acc| acc.nonce.map_or_else(|| true, |nonce| tx.nonce() == nonce + 1))
    }

    pub fn get_tx_by_hash(&self, tx_hash: &TxHash) -> Option<&MempoolTx<LoomDataTypesEthereum>> {
        self.txs.get(tx_hash)
    }

    pub fn get_or_fetch_pre_state(&mut self, _tx_hash: &TxHash) -> Result<FetchState<GethStateUpdate>> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }

    pub fn remove_tx(&mut self, tx_hash: &TxHash) -> Option<MempoolTx<LoomDataTypesEthereum>> {
        self.txs.remove(tx_hash)
    }
}
