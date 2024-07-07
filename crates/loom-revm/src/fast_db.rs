use std::collections::{BTreeMap, HashMap};
use std::ops::{Deref, DerefMut};

use alloy::{
    primitives::U256,
    rpc::types::trace::geth::AccountState,
};
use revm::{Database, DatabaseCommit, DatabaseRef, InMemoryDB};
use revm::db::{AccountState as DbAccountState, CacheDB, EmptyDB};
use revm::precompile::Address;
use revm::primitives::{AccountInfo, Bytecode, KECCAK_EMPTY};
use revm::primitives::bitvec::macros::internal::funty::Fundamental;

use crate::fast_inmemorydb::FastInMemoryDb;

#[derive(Clone, Default)]
pub struct FastDb<ExtDB> {
    in_memory_db: ExtDB,
    update: BTreeMap<Address, AccountState>,
}

impl<ExtDB> FastDb<ExtDB>
where
    ExtDB: DatabaseRef + DatabaseCommit + Database,
{
    pub fn empty() -> FastDb<InMemoryDB> {
        FastDb {
            in_memory_db: InMemoryDB::new(EmptyDB::new()),
            update: BTreeMap::new(),
        }
    }

    pub fn with_db(self, in_memory_db: ExtDB) -> FastDb<ExtDB> {
        FastDb {
            in_memory_db,
            ..self
        }
    }

    pub fn with_update(self, update: BTreeMap<Address, AccountState>) -> FastDb<ExtDB> {
        FastDb {
            update,
            ..self
        }
    }

    pub fn with_update_vec(self, update_vec: Vec<BTreeMap<Address, AccountState>>) -> FastDb<ExtDB> {
        let mut update: BTreeMap<Address, AccountState> = BTreeMap::new();

        for update_entry in update_vec {
            for (account, update_state) in update_entry {
                let mut account_state = update.entry(account).or_default();
                if update_state.code.is_some() {
                    account_state.code = update_state.code
                }
                if update_state.balance.is_some() {
                    account_state.balance = update_state.balance
                }
                if update_state.nonce.is_some() {
                    account_state.nonce = update_state.nonce
                }
                for (slot, value) in update_state.storage {
                    account_state.storage.insert(slot, value);
                }
            }
        }

        FastDb {
            update,
            ..self
        }
    }
}
/*
impl<ExtDB> Database for FastDb<ExtDB>
where
    ExtDB: DatabaseRef<Error=Infallible> + Send + Sync + Clone + 'static,
{
    type Error = Infallible;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic(address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0.code_by_hash(code_hash)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.0.storage(address, index)
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.0.block_hash(number)
    }
}

impl<ExtDB> DatabaseRef for FastDb<ExtDB>
where
    ExtDB: DatabaseRef<Error=Infallible> + Send + Sync + Clone + 'static,
{
    type Error = Infallible;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic_ref(address)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0.code_by_hash_ref(code_hash)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.0.storage_ref(address, index)
    }

    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        self.0.block_hash_ref(number)
    }
}

impl<ExtDB> DatabaseCommit for FastDb<ExtDB>
where
    ExtDB: DatabaseRef + Send + Sync + Clone + 'static,
{
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        self.0.commit(changes)
    }
}

 */

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::primitives::Bytes;
    use rand::{Rng, thread_rng};
    use revm::db::DbAccount;
    use revm::primitives::KECCAK_EMPTY;

    use super::*;

    fn generate_account(mem_size: usize) -> DbAccount {
        let mut rng = thread_rng();
        let mut storage: HashMap<U256, U256> = HashMap::new();
        for _j in 0..mem_size {
            storage.insert(rng.gen::<U256>(), rng.gen::<U256>());
        }

        let code = rng.gen::<U256>();

        let info = AccountInfo::new(U256::ZERO, 0, KECCAK_EMPTY, Bytecode::new_raw(Bytes::from(code.to_be_bytes_vec())));

        let acc = DbAccount {
            info,
            account_state: DbAccountState::Touched,
            storage,
        };
        acc
    }

    fn generate_accounts(acc_size: usize, mem_size: usize) -> Vec<DbAccount> {
        let mut rng = thread_rng();
        let mut ret: Vec<DbAccount> = Vec::new();
        for _i in 0..acc_size {
            ret.push(generate_account(mem_size));
        }
        ret
    }

    #[test]
    fn test_speed() {
        let mut db0 = CacheDB::new(EmptyDB::new());

        let n = 100000;
        let n2 = 1000;

        let accs = generate_accounts(n, 100);
        let addr: Vec<Address> = (0..n).map(|x| Address::random()).collect();


        for a in 0..n {
            db0.accounts.insert(addr[a], accs[a].clone());
        }

        let accs2 = generate_accounts(n2, 100);
        let start_time = chrono::Local::now();

        for (i, a) in accs2.into_iter().enumerate() {
            db0.accounts.insert(addr[i], a);
        }


        println!("Write {n2} {}", chrono::Local::now() - start_time);

        let start_time = chrono::Local::now();

        for i in 0..n2 {
            let acc = db0.load_account(addr[i]).unwrap();
        }

        println!("Read {n2} {}", chrono::Local::now() - start_time);

        let mut db1 = FastInMemoryDb::with_ext_db(Arc::new(db0));

        let start_time = chrono::Local::now();

        for i in 0..n2 {
            let acc = db1.load_account(addr[i]).unwrap();
        }

        println!("Read known {n2} {}", chrono::Local::now() - start_time);

        let start_time = chrono::Local::now();

        for i in n2..n2 + n2 {
            let acc = db1.load_account(addr[i]).unwrap();
        }

        println!("Read unknown {n2} {}", chrono::Local::now() - start_time);
    }

    #[test]
    fn db_read() {
        let acc = generate_account(100);
        let addr = Address::random();
        let mut db0 = CacheDB::new(EmptyDB::new());
        db0.accounts.insert(addr, acc.clone());
        let mut db1 = FastInMemoryDb::with_ext_db(Arc::new(db0));
        let acc_ref = db1.basic_ref(addr).unwrap();
        println!("{:?}", acc_ref);
        let acc_ref = db1.basic_ref(Address::random()).unwrap();
        println!("{:?}", acc_ref);
    }
}
