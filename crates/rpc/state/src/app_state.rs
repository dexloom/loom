use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_storage_db::DbPool;
use revm::{DatabaseCommit, DatabaseRef};

#[derive(Clone)]
pub struct AppState<DB: DatabaseRef + DatabaseCommit + Clone + Send + Sync + 'static> {
    pub db: DbPool,
    pub bc: Blockchain,
    pub state: BlockchainState<DB>,
}
