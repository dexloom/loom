use loom_core_blockchain::Blockchain;
use loom_storage_db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub bc: Blockchain,
}
