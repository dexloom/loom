use alloy::primitives::{Address, BlockNumber, B256, U256};
use eyre::{eyre, ErrReport};
use revm::primitives::{AccountInfo, Bytecode};
use revm::DatabaseRef;

pub struct LoomDBHelper {}

impl LoomDBHelper {
    #[inline]
    pub fn get_code_by_hash<DB: DatabaseRef<Error = ErrReport>>(read_only_db: &Option<DB>, code_hash: B256) -> eyre::Result<Bytecode> {
        match read_only_db {
            Some(read_only_db) => read_only_db.code_by_hash_ref(code_hash).map_err(|_| eyre!("CODE_HASH_NOT_FOUND")),
            None => Err(eyre!("NO_RO_DB")),
        }
    }

    #[inline]
    fn fetch_storage<ExtDB: DatabaseRef<Error = ErrReport>>(ext_db: &Option<ExtDB>, address: Address, index: U256) -> eyre::Result<U256> {
        if let Some(ext_db) = ext_db {
            ext_db.storage_ref(address, index).map_err(|_| eyre!("ERROR_READING_ALLOY_DB"))
        } else {
            Err(eyre!("NO_EXT_DB"))
        }
    }

    #[inline]
    pub fn get_or_fetch_storage<DB: DatabaseRef<Error = ErrReport>, ExtDB: DatabaseRef<Error = ErrReport>>(
        read_only_db: &Option<DB>,
        ext_db: &Option<ExtDB>,
        address: Address,
        index: U256,
    ) -> eyre::Result<U256> {
        match read_only_db {
            Some(read_only_db) => {
                read_only_db.storage_ref(address, index).or_else(|_| Ok(Self::fetch_storage(ext_db, address, index).unwrap_or_default()))
            }
            None => Ok(Self::fetch_storage(ext_db, address, index).unwrap_or_default()),
        }
    }

    #[inline]
    fn fetch_basic<ExtDB: DatabaseRef<Error = ErrReport>>(ext_db: &Option<ExtDB>, address: Address) -> eyre::Result<Option<AccountInfo>> {
        if let Some(ext_db) = ext_db {
            ext_db.basic_ref(address).map_err(|_| eyre!("ERROR_READING_EXT_DB"))
        } else {
            Err(eyre!("NO_EXT_DB"))
        }
    }

    #[inline]
    pub fn get_basic<DB: DatabaseRef<Error = ErrReport>>(read_only_db: &Option<DB>, address: Address) -> eyre::Result<Option<AccountInfo>> {
        read_only_db
            .as_ref()
            .and_then(|db| db.basic_ref(address).ok().flatten())
            .map_or_else(|| Err(eyre!("NO_ACCOUNT")), |info| Ok(Some(info)))
    }

    #[inline]
    pub fn get_or_fetch_basic<DB: DatabaseRef<Error = ErrReport>, ExtDB: DatabaseRef<Error = ErrReport>>(
        read_only_db: &Option<DB>,
        ext_db: &Option<ExtDB>,
        address: Address,
    ) -> eyre::Result<Option<AccountInfo>> {
        match &read_only_db {
            Some(read_only_db) => match read_only_db.basic_ref(address) {
                Ok(Some(info)) => Ok(Some(info)),
                _ => Self::fetch_basic(ext_db, address),
            },
            None => Self::fetch_basic(ext_db, address),
        }
    }

    #[inline]
    fn fetch_block_hash_ref<ExtDB: DatabaseRef<Error = ErrReport>>(ext_db: &Option<ExtDB>, number: BlockNumber) -> eyre::Result<B256> {
        if let Some(ext_db) = ext_db {
            ext_db.block_hash_ref(number).map_err(|_| eyre!("ERROR_READING_EXT_DB"))
        } else {
            Err(eyre!("NO_EXT_DB"))
        }
    }

    #[inline]
    pub fn get_or_fetch_block_hash<DB: DatabaseRef<Error = ErrReport>, ExtDB: DatabaseRef<Error = ErrReport>>(
        read_only_db: &Option<DB>,
        ext_db: &Option<ExtDB>,
        number: BlockNumber,
    ) -> eyre::Result<B256> {
        match read_only_db {
            Some(read_only_db) => read_only_db.block_hash_ref(number).or_else(|_| LoomDBHelper::fetch_block_hash_ref(ext_db, number)),
            None => Self::fetch_block_hash_ref(ext_db, number),
        }
    }
}
