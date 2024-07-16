use std::sync::Arc;

use revm::db::EmptyDB;

use crate::fast_cache_db::FastCacheDB;

pub type LoomInMemoryDB = FastCacheDB<Arc<FastCacheDB<EmptyDB>>>;
