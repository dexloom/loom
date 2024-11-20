use loom_core_blockchain::{Blockchain, LoomDataTypesEthereum};
use loom_storage_db::DbPool;
use revm::DatabaseRef;

#[derive(Clone)]
pub struct AppState<DB: DatabaseRef + Clone + Send + Sync + 'static> {
    pub db: DbPool,
    pub bc: Blockchain<DB, LoomDataTypesEthereum>,
}
