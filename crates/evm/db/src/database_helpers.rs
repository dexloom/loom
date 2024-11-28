use alloy::primitives::map::HashMap;
use alloy::primitives::{Address, U256};
use alloy::rpc::types::trace::geth::AccountState;
use revm::db::DbAccount;
use revm::primitives::{Account, AccountStatus, Bytecode, EvmStorageSlot};
use revm::{DatabaseCommit, DatabaseRef};
use std::collections::BTreeMap;
use tracing::trace;

pub struct DatabaseHelpers {}

impl DatabaseHelpers {
    #[inline]
    pub fn account_db_to_revm(db: DbAccount) -> Account {
        let storage = db.storage.into_iter().map(|(k, v)| (k, EvmStorageSlot::new(v))).collect();
        Account { info: db.info, storage, status: AccountStatus::Touched }
    }

    #[inline]
    pub fn trace_update_to_commit_update<DB: DatabaseRef>(db: &DB, update: BTreeMap<Address, AccountState>) -> HashMap<Address, Account> {
        let mut result: HashMap<Address, Account> = Default::default();
        for (address, state) in update {
            trace!(%address, code=state.code.is_some(), storage=state.storage.len(), "trace_update_to_commit_update");
            if address.is_zero() {
                continue;
            }
            let mut info = db.basic_ref(address).map(|a| a.unwrap_or_default()).unwrap_or_default();

            if let Some(code) = state.code {
                let code = Bytecode::new_raw(code);
                let hash = code.hash_slow();
                info.code_hash = hash;
                info.code = Some(code);
            }

            if let Some(nonce) = state.nonce {
                info.nonce = nonce
            }

            if let Some(balance) = state.balance {
                info.balance = balance
            }

            let storage = state.storage.into_iter().map(|(k, v)| (k.into(), EvmStorageSlot::new_changed(U256::ZERO, v.into()))).collect();

            result.insert(address, Account { info, storage, status: AccountStatus::Touched });
        }
        result
    }
    #[inline]
    pub fn apply_geth_state_update<DB: DatabaseRef + DatabaseCommit>(db: &mut DB, update: BTreeMap<Address, AccountState>) {
        let update = Self::trace_update_to_commit_update(db, update);
        db.commit(update);
    }

    #[inline]
    pub fn apply_geth_state_update_vec<DB: DatabaseRef + DatabaseCommit>(db: &mut DB, update_vec: Vec<BTreeMap<Address, AccountState>>) {
        let mut update_map: HashMap<Address, Account> = Default::default();

        for update in update_vec {
            let update_record = Self::trace_update_to_commit_update(db, update);
            update_map.extend(update_record);
        }
        db.commit(update_map)
    }
}
