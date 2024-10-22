use crate::alloydb::AlloyDB;
use crate::fast_cache_db::FastCacheDB;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{Address, B256, U256};
use alloy::providers::RootProvider;
use alloy::rpc::types::trace::geth::AccountState as GethAccountState;
use alloy::transports::BoxTransport;
use eyre::{eyre, ErrReport};
use revm::db::{AccountState, EmptyDB};
use revm::primitives::{AccountInfo, Bytecode};
use revm::DatabaseRef;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tracing::{error, trace};

#[derive(Clone, Default)]
pub enum ExtDB {
    #[default]
    Empty,
    RPC(RootProvider<BoxTransport>),
}

impl fmt::Debug for ExtDB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtDB::Empty => {
                write!(f, "EmptyDB")
            }
            ExtDB::RPC(_) => {
                write!(f, "RPC")
            }
        }
    }
}

impl DatabaseRef for ExtDB {
    type Error = ErrReport;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        match self.clone() {
            ExtDB::Empty => Ok(None),
            ExtDB::RPC(provider) => match AlloyDB::new(provider, BlockNumberOrTag::Latest.into()) {
                Some(alloy_db) => alloy_db.basic_ref(address).map_err(|_| eyre!("ERROR_READING_BASIC_REF")),
                None => Err(eyre!("ALLOY_DB_NOT_CREATED")),
            },
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        Err(eyre!("NE"))
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Err(eyre!("NE"))
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        Err(eyre!("NE"))
    }
}

pub type LoomInMemoryDB = FastCacheDB<Arc<FastCacheDB<EmptyDB>>>;

/*impl Default for LoomInMemoryDB {
    fn default() -> Self {
        FastCacheDB::new(Arc::new(FastCacheDB::new(Default::default())))
    }
}

 */

/*impl Deref for LoomInMemoryDB {
    type Target = FastCacheDB<Arc<FastCacheDB<ExtDB>>>;
    fn deref(&self) -> &Self::Target {
        self
    }
}

 */

//pub type FastInMemoryDB = FastCacheDB<Arc<FastCacheDB<EmptyDB>>>;

impl LoomInMemoryDB {
    pub fn with_db(self, db: Arc<FastCacheDB<EmptyDB>>) -> Self {
        Self { db, ..self }
    }

    pub fn merge(&self) -> FastCacheDB<EmptyDB> {
        let mut db: FastCacheDB<EmptyDB> = FastCacheDB {
            accounts: self.db.accounts.clone(),
            logs: self.db.logs.clone(),
            contracts: self.db.contracts.clone(),
            block_hashes: self.db.block_hashes.clone(),
            db: self.db.db.clone(),
        };
        for (k, v) in self.block_hashes.iter() {
            db.block_hashes.insert(*k, *v);
        }
        for (k, v) in self.contracts.iter() {
            db.contracts.insert(*k, v.clone());
        }
        db.logs.clone_from(&self.logs);
        for (address, account) in self.accounts.iter() {
            let mut info = account.info.clone();
            db.insert_contract(&mut info);

            let entry = db.accounts.entry(*address).or_default();
            entry.info = info;
            for (k, v) in account.storage.iter() {
                entry.storage.insert(*k, *v);
            }
        }
        db
    }

    pub fn update_accounts(&self) -> FastCacheDB<EmptyDB> {
        let mut db = (*self.db.as_ref()).clone();

        for (k, v) in self.block_hashes.iter() {
            db.block_hashes.insert(*k, *v);
        }
        for (k, v) in self.contracts.iter() {
            db.contracts.entry(*k).and_modify(|k| k.clone_from(v));
        }
        db.logs.clone_from(&self.logs);

        for (address, account) in self.accounts.iter() {
            db.accounts.entry(*address).and_modify(|db_account| {
                let info = account.info.clone();
                db_account.info = info;
                for (k, v) in account.storage.iter() {
                    db_account.storage.insert(*k, *v);
                }
                db_account.account_state = AccountState::Touched
            });
        }
        db
    }

    pub fn update_cells(&self) -> FastCacheDB<EmptyDB> {
        let mut db = self.db.as_ref().clone();

        for (k, v) in self.block_hashes.iter() {
            db.block_hashes.insert(*k, *v);
        }
        for (k, v) in self.contracts.iter() {
            db.contracts.entry(*k).and_modify(|k| k.clone_from(v));
        }
        db.logs.clone_from(&self.logs);

        for (address, account) in self.accounts.iter() {
            db.accounts.entry(*address).and_modify(|db_account| {
                let info = account.info.clone();
                db_account.info = info;
                for (k, v) in account.storage.iter() {
                    db_account.storage.entry(*k).and_modify(|cv| cv.clone_from(v));
                }
                db_account.account_state = AccountState::Touched
            });
        }
        db
    }

    pub fn apply_geth_update(&mut self, update: BTreeMap<Address, GethAccountState>) {
        for (addr, acc_state) in update {
            trace!("apply_geth_update {} is code {} storage_len {} ", addr, acc_state.code.is_some(), acc_state.storage.len());

            for (k, v) in acc_state.storage.iter() {
                if let Err(e) = self.insert_account_storage(addr, (*k).into(), (*v).into()) {
                    error!("apply_geth_update :{}", e);
                }
            }
            if let Ok(account) = self.load_account(addr) {
                if let Some(code) = acc_state.code.clone() {
                    let bytecode = Bytecode::new_raw(code);
                    account.info.code_hash = bytecode.hash_slow();
                    account.info.code = Some(bytecode);
                }
                if let Some(nonce) = acc_state.nonce {
                    //trace!("nonce : {} -> {}", account.info.nonce, nonce);
                    account.info.nonce = nonce;
                }
                if let Some(balance) = acc_state.balance {
                    account.info.balance = balance;
                }
                account.account_state = AccountState::Touched;
            }
        }
    }

    pub fn apply_geth_update_vec(&mut self, update: Vec<BTreeMap<Address, GethAccountState>>) {
        for entry in update.into_iter() {
            self.apply_geth_update(entry);
        }
    }
}
