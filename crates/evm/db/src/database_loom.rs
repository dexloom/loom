use crate::fast_cache_db::FastDbAccount;
use alloy::primitives::map::HashMap;
use alloy::primitives::{Address, U256};
use eyre::ErrReport;
use revm::primitives::AccountInfo;
use revm::DatabaseRef;

pub trait DatabaseLoomExt {
    fn with_ext_db(&mut self, db: impl DatabaseRef<Error = ErrReport> + Send + Sync + 'static);
    fn is_account(&self, address: &Address) -> bool;
    fn is_slot(&self, address: &Address, slot: &U256) -> bool;
    fn contracts_len(&self) -> usize;
    fn accounts_len(&self) -> usize;
    fn storage_len(&self) -> usize;

    fn load_account(&mut self, address: Address) -> eyre::Result<&mut FastDbAccount>;

    fn load_cached_account(&mut self, address: Address) -> eyre::Result<&mut FastDbAccount>;

    fn insert_contract(&mut self, account: &mut AccountInfo);

    fn insert_account_info(&mut self, address: Address, info: AccountInfo);

    fn insert_account_storage(&mut self, address: Address, slot: U256, value: U256) -> eyre::Result<()>;

    fn replace_account_storage(&mut self, address: Address, storage: HashMap<U256, U256>) -> eyre::Result<()>;

    fn maintain(self) -> Self;
}
