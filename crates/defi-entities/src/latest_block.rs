use std::collections::HashMap;

use alloy_primitives::{Address, BlockHash, BlockNumber, B256, U64};
use alloy_rpc_types::state::{AccountOverride, StateOverride};
use alloy_rpc_types::{Block, BlockTransactions, Header, Log, Transaction};

use defi_types::GethStateUpdateVec;

pub struct LatestBlock {
    pub block_number: BlockNumber,
    pub block_hash: BlockHash,
    pub block_header: Option<Header>,
    pub block_with_txs: Option<Block>,
    pub logs: Option<Vec<Log>>,
    pub diff: Option<GethStateUpdateVec>,
}

impl LatestBlock {
    pub fn hash(&self) -> BlockHash {
        self.block_hash
    }
    pub fn number(&self) -> BlockNumber {
        self.block_number
    }

    pub fn number_and_hash(&self) -> (BlockNumber, BlockHash) {
        (self.block_number, self.block_hash)
    }

    pub fn new(block_number: BlockNumber, block_hash: BlockHash) -> Self {
        Self { block_number, block_hash, block_header: None, block_with_txs: None, logs: None, diff: None }
    }

    pub fn node_state_override(&self) -> StateOverride {
        let mut cur_state_override = StateOverride::default();

        if let Some(cur_diff) = self.diff.as_ref() {
            for diff_entry in cur_diff {
                for (addr, state) in diff_entry {
                    let account = cur_state_override.entry(*addr).or_insert(AccountOverride::default());
                    account.balance = state.balance;
                    account.nonce = state.nonce.map(|n| U64::from(n));

                    let diff: HashMap<B256, B256> = state.storage.iter().map(|(k, v)| (*k, (*v))).collect();
                    account.state_diff = Some(diff);
                }
            }
        }
        cur_state_override
    }

    pub fn txs(&self) -> Option<&Vec<Transaction>> {
        if let Some(block) = &self.block_with_txs {
            if let BlockTransactions::Full(txs) = &block.transactions {
                return Some(txs);
            }
        }
        None
    }

    pub fn coinbase(&self) -> Option<Address> {
        if let Some(block) = &self.block_with_txs {
            return Some(block.header.miner);
        }
        None
    }

    pub fn update(
        &mut self,
        block_number: BlockNumber,
        block_hash: BlockHash,
        block_header: Option<Header>,
        block_with_txes: Option<Block>,
        logs: Option<Vec<Log>>,
        diff: Option<GethStateUpdateVec>,
    ) -> bool {
        if block_number >= self.block_number {
            let is_new = block_number > self.block_number;

            if block_number > self.block_number || block_hash != self.block_hash {
                *self = Self::new(block_number, block_hash);
            }

            if let Some(x) = block_header {
                self.block_header = Some(x);
            }
            if let Some(x) = block_with_txes {
                self.block_with_txs = Some(x);
            }
            if let Some(x) = logs {
                self.logs = Some(x);
            }
            if let Some(x) = diff {
                self.diff = Some(x)
            }

            is_new
        } else {
            false
        }
    }
}
