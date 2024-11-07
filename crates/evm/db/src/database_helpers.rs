use alloy::primitives::map::HashMap;
use alloy::primitives::Address;
use alloy::rpc::types::trace::geth::AccountState;
use eyre::ErrReport;
use revm::primitives::Account;
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::collections::BTreeMap;

fn trace_update_to_commit_update(update: BTreeMap<Address, AccountState>) -> HashMap<Address, Account> {
    Default::default()
}

pub struct DatabaseHelpers {}

// TODO Implement
impl DatabaseHelpers {
    #[inline]
    pub fn apply_geth_state_update<DB: DatabaseCommit>(db: &mut DB, update: BTreeMap<Address, AccountState>) {
        let update = trace_update_to_commit_update(update);
        db.commit(update);
    }

    #[inline]
    pub fn apply_geth_state_update_vec<DB: DatabaseCommit>(db: &mut DB, update_vec: Vec<BTreeMap<Address, AccountState>>) {
        for update in update_vec {
            Self::apply_geth_state_update(db, update)
        }
    }
}
