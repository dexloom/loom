use alloy_primitives::{Address, BlockHash, BlockNumber, U256};
use loom_evm_db::DatabaseHelpers;
use loom_types_blockchain::{GethStateUpdate, GethStateUpdateVec};
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default)]
pub struct MarketStateConfig {
    pub force_insert_accounts: HashSet<Address>,
    pub read_only_cells: HashMap<Address, HashSet<U256>>,
}

impl MarketStateConfig {
    pub fn is_force_insert(&self, address: &Address) -> bool {
        self.force_insert_accounts.contains(address)
    }

    pub fn add_force_insert(&mut self, address: Address) {
        self.force_insert_accounts.insert(address);
    }

    pub fn disable_cell(&mut self, address: Address, cell: U256) {
        self.read_only_cells.entry(address).or_default().insert(cell);
    }

    pub fn disable_cell_vec(&mut self, address: Address, cells: Vec<U256>) {
        for cell in cells {
            self.disable_cell(address, cell)
        }
    }

    pub fn is_read_only_cell(&self, address: &Address, cell: &U256) -> bool {
        match self.read_only_cells.get(address) {
            Some(hashset) => hashset.contains(cell),
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct MarketState<DB> {
    pub block_number: BlockNumber,
    pub block_hash: BlockHash,
    pub state_db: DB,
    pub config: MarketStateConfig,
}

impl<DB: DatabaseRef + Database + DatabaseCommit> MarketState<DB> {
    pub fn new(db: DB) -> MarketState<DB> {
        MarketState { block_number: Default::default(), block_hash: Default::default(), state_db: db, config: Default::default() }
    }

    pub fn hash(&self) -> BlockHash {
        self.block_hash
    }
    pub fn number(&self) -> BlockNumber {
        self.block_number
    }

    pub fn number_and_hash(&self) -> (BlockNumber, BlockHash) {
        (self.block_number, self.block_hash)
    }

    pub fn apply_geth_update(&mut self, update: GethStateUpdate) {
        DatabaseHelpers::apply_geth_state_update(&mut self.state_db, update)
    }

    pub fn apply_geth_update_vec(&mut self, update: GethStateUpdateVec) {
        for entry in update {
            self.apply_geth_update(entry)
        }
    }

    // pub fn add_state(&mut self, state: &GethStateUpdate) {
    //     for (address, account_state) in state.iter() {
    //         let hex_code = account_state.code.as_ref().map(|code_bytes| Bytecode::new_raw(code_bytes.clone()));
    //
    //         let balance: U256 = account_state.balance.unwrap_or_default();
    //
    //         let nonce = account_state.nonce.unwrap_or_default();
    //
    //         trace!("Address {:#20x} Code : {}", address, hex_code.is_some());
    //
    //         let account_info = AccountInfo {
    //             balance,
    //             nonce,
    //             code_hash: if hex_code.is_some() { KECCAK_EMPTY } else { Default::default() },
    //             code: hex_code,
    //         };
    //
    //         self.state_db.insert_account_info(*address, account_info);
    //         for (slot, value) in account_state.storage.iter() {
    //             self.state_db.insert_account_storage(*address, (*slot).into(), (*value).into()).unwrap();
    //             trace!("Contract {} Storage {} = {}", address, slot, value);
    //         }
    //     }
    //
    //     //debug!("Added state : {}", state.len());
    // }
}
