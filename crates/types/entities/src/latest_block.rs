use alloy_primitives::map::B256HashMap;
use alloy_primitives::{Address, BlockHash, BlockNumber, B256};
use alloy_rpc_types::state::{AccountOverride, StateOverride};
use alloy_rpc_types::Header;

use loom_types_blockchain::{GethStateUpdate, GethStateUpdateVec, LoomBlock, LoomHeader};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};

pub struct LatestBlock<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub block_number: BlockNumber,
    pub block_hash: LDT::BlockHash,
    pub block_header: Option<LDT::Header>,
    pub block_with_txs: Option<LDT::Block>,
    pub logs: Option<Vec<LDT::Log>>,
    pub diff: Option<Vec<LDT::StateUpdate>>,
}

impl<LDT: LoomDataTypes<Address = Address, Header = Header, BlockHash = BlockHash, StateUpdate = GethStateUpdate>> LatestBlock<LDT> {
    pub fn hash(&self) -> LDT::BlockHash {
        self.hash()
    }

    pub fn parent_hash(&self) -> Option<LDT::BlockHash> {
        self.block_header.as_ref().map(|x| <Header as LoomHeader<LDT>>::get_parent_hash(x))
    }
    pub fn number(&self) -> BlockNumber {
        self.block_number
    }

    pub fn number_and_hash(&self) -> (BlockNumber, BlockHash) {
        (self.block_number, self.block_hash)
    }

    pub fn new(block_number: BlockNumber, block_hash: LDT::BlockHash) -> Self {
        Self { block_number, block_hash, block_header: None, block_with_txs: None, logs: None, diff: None }
    }

    pub fn node_state_override(&self) -> StateOverride {
        let mut cur_state_override = StateOverride::default();

        if let Some(cur_diff) = self.diff.as_ref() {
            for diff_entry in cur_diff {
                for (addr, state) in diff_entry {
                    let account = cur_state_override.entry(*addr).or_insert(AccountOverride::default());
                    account.balance = state.balance;
                    account.nonce = state.nonce;

                    let diff: B256HashMap<B256> = state.storage.iter().map(|(k, v)| (*k, *v)).collect();
                    account.state_diff = Some(diff);
                }
            }
        }
        cur_state_override
    }

    pub fn txs(&self) -> Option<Vec<LDT::Transaction>> {
        if let Some(block) = &self.block_with_txs {
            Some(block.get_transactions())
        } else {
            None
        }
    }

    pub fn coinbase(&self) -> Option<LDT::Address> {
        if let Some(block) = &self.block_with_txs {
            return Some(<alloy_rpc_types::Header as LoomHeader<LDT>>::get_beneficiary(&block.get_header()));
        }
        None
    }

    pub fn update(
        &mut self,
        block_number: BlockNumber,
        block_hash: LDT::BlockHash,
        block_header: Option<LDT::Header>,
        block_with_txes: Option<LDT::Block>,
        logs: Option<Vec<LDT::Log>>,
        diff: Option<Vec<LDT::StateUpdate>>,
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
