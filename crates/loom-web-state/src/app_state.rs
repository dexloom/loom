use defi_blockchain::Blockchain;
use loom_db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub bc: Blockchain,
}
