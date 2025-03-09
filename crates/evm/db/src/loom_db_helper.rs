use crate::error::LoomDBError;
use alloy::primitives::{Address, BlockNumber, B256, U256};
use eyre::eyre;
use revm::state::{AccountInfo, Bytecode};
use revm::DatabaseRef;
use tracing::trace;

pub struct LoomDBHelper {}

impl LoomDBHelper {
    #[inline]
    pub fn get_code_by_hash<DB: DatabaseRef<Error = LoomDBError>>(
        read_only_db: &Option<DB>,
        code_hash: B256,
    ) -> eyre::Result<Bytecode, LoomDBError> {
        match read_only_db {
            Some(read_only_db) => read_only_db.code_by_hash_ref(code_hash),
            None => Err(LoomDBError::NoDB),
        }
    }

    #[inline]
    fn fetch_storage<ExtDB: DatabaseRef<Error = LoomDBError>>(
        ext_db: &Option<ExtDB>,
        address: Address,
        slot: U256,
    ) -> Result<U256, LoomDBError> {
        trace!(%address, %slot, "fetch_storage");

        if let Some(ext_db) = ext_db {
            let value = ext_db.storage_ref(address, slot).map_err(|_| eyre!("ERROR_READING_ALLOY_DB"));
            trace!(%address, %slot, ?value , "fetch_storage returned");
            Ok(value.unwrap_or_default())
        } else {
            trace!("fetch_storage returned NO_EXT_DB");
            Err(LoomDBError::NoDB)
        }
    }

    #[inline]
    pub fn get_or_fetch_storage<DB: DatabaseRef<Error = LoomDBError>, ExtDB: DatabaseRef<Error = LoomDBError>>(
        read_only_db: &Option<DB>,
        ext_db: &Option<ExtDB>,
        address: Address,
        slot: U256,
    ) -> Result<U256, LoomDBError> {
        trace!(%address, %slot, "get_or_fetch_storage");

        match read_only_db {
            Some(read_only_db) => {
                let value = read_only_db.storage_ref(address, slot).or_else(|_| Self::fetch_storage(ext_db, address, slot));
                trace!(%address, %slot, ?value , "get_or_fetch_storage with RO");
                value
            }
            None => {
                let value = Self::fetch_storage(ext_db, address, slot);
                trace!(%address, %slot, ?value , "get_or_fetch_storage without RO");
                value
            }
        }
    }

    #[inline]
    fn fetch_basic<ExtDB: DatabaseRef<Error = LoomDBError>>(
        ext_db: &Option<ExtDB>,
        address: Address,
    ) -> Result<Option<AccountInfo>, LoomDBError> {
        trace!(%address, "fetch_basic");
        if let Some(ext_db) = ext_db {
            ext_db.basic_ref(address)
        } else {
            Err(LoomDBError::NoDB)
        }
    }

    #[inline]
    pub fn get_basic<DB: DatabaseRef<Error = LoomDBError>>(
        read_only_db: &Option<DB>,
        address: Address,
    ) -> eyre::Result<Option<AccountInfo>> {
        read_only_db
            .as_ref()
            .and_then(|db| db.basic_ref(address).ok().flatten())
            .map_or_else(|| Err(eyre!("NO_ACCOUNT")), |info| Ok(Some(info)))
    }

    #[inline]
    pub fn get_or_fetch_basic<DB: DatabaseRef<Error = LoomDBError>, ExtDB: DatabaseRef<Error = LoomDBError>>(
        read_only_db: &Option<DB>,
        ext_db: &Option<ExtDB>,
        address: Address,
    ) -> Result<Option<AccountInfo>, LoomDBError> {
        trace!(%address, "get_or_fetch_basic");
        match &read_only_db {
            Some(read_only_db) => match read_only_db.basic_ref(address) {
                Ok(Some(info)) => Ok(Some(info)),
                _ => Self::fetch_basic(ext_db, address),
            },
            None => Self::fetch_basic(ext_db, address),
        }
    }

    #[inline]
    fn fetch_block_hash_ref<ExtDB: DatabaseRef<Error = LoomDBError>>(
        ext_db: &Option<ExtDB>,
        number: BlockNumber,
    ) -> Result<B256, LoomDBError> {
        if let Some(ext_db) = ext_db {
            ext_db.block_hash_ref(number)
        } else {
            Err(LoomDBError::NoDB)
        }
    }

    #[inline]
    pub fn get_or_fetch_block_hash<DB: DatabaseRef<Error = LoomDBError>, ExtDB: DatabaseRef<Error = LoomDBError>>(
        read_only_db: &Option<DB>,
        ext_db: &Option<ExtDB>,
        number: BlockNumber,
    ) -> Result<B256, LoomDBError> {
        match read_only_db {
            Some(read_only_db) => read_only_db.block_hash_ref(number).or_else(|_| LoomDBHelper::fetch_block_hash_ref(ext_db, number)),
            None => Self::fetch_block_hash_ref(ext_db, number),
        }
    }
}
