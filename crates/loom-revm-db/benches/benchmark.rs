use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash};

use alloy::primitives::{Address, Bytes, U256};
use criterion::{criterion_group, criterion_main, Criterion};
use rand::{thread_rng, Rng};
use revm::db::{AccountState as DbAccountState, CacheDB, DbAccount, EmptyDB};
use revm::primitives::{AccountInfo, Bytecode, KECCAK_EMPTY};
use revm::DatabaseRef;

use loom_revm_db::fast_hasher::{HashedAddress, HashedAddressCell, SimpleBuildHasher, SimpleHasher};
use loom_revm_db::LoomInMemoryDB;

const N: usize = 100000;
const N_ACC: usize = 10000;
const N_MEM: usize = 1000;

fn generate_account(mem_size: usize) -> DbAccount {
    let mut rng = thread_rng();
    let mut storage: HashMap<U256, U256> = HashMap::new();
    for _j in 0..mem_size {
        storage.insert(rng.gen::<U256>(), rng.gen::<U256>());
    }

    let code = rng.gen::<U256>();

    let info = AccountInfo::new(U256::ZERO, 0, KECCAK_EMPTY, Bytecode::new_raw(Bytes::from(code.to_be_bytes_vec())));

    DbAccount { info, account_state: DbAccountState::Touched, storage }
}
fn generate_accounts(acc_size: usize, mem_size: usize) -> Vec<DbAccount> {
    let mut ret: Vec<DbAccount> = Vec::new();
    for _i in 0..acc_size {
        ret.push(generate_account(mem_size));
    }
    ret
}

fn fill_cache_db(db: &mut CacheDB<EmptyDB>, addr: &[Address], accs: &[DbAccount]) {
    for a in 0..addr.len() {
        db.insert_account_info(addr[a], accs[a].info.clone());
        for (k, v) in accs[a].storage.iter() {
            let _ = db.insert_account_storage(addr[a], *k, *v);
        }
    }
}

fn fill_loom_db(db: &mut LoomInMemoryDB, addr: &[Address], accs: &[DbAccount]) {
    for a in 0..addr.len() {
        db.insert_account_info(addr[a], accs[a].info.clone());
        for (k, v) in accs[a].storage.iter() {
            let _ = db.insert_account_storage(addr[a], *k, *v);
        }
    }
}

fn test_insert_cache_db(addr: &[Address], accs: &[DbAccount]) {
    let mut db = CacheDB::new(EmptyDB::new());
    fill_cache_db(&mut db, addr, accs);
}

fn test_insert_loom_db(addr: &[Address], accs: &[DbAccount]) {
    let mut db = LoomInMemoryDB::default();
    fill_loom_db(&mut db, addr, accs);
}

fn test_read_cache_db(db: &CacheDB<EmptyDB>, addr: &[Address], accs: &[DbAccount]) {
    for (i, a) in addr.iter().enumerate() {
        for (k, v) in accs[i].storage.iter() {
            if db.storage_ref(*a, *k).unwrap() != *v {
                panic!("BAD_VALUE")
            }
        }
    }
}

fn test_read_loom_db(db: &LoomInMemoryDB, addr: &[Address], accs: &[DbAccount]) {
    for (i, a) in addr.iter().enumerate() {
        for (k, v) in accs[i].storage.iter() {
            if db.storage_ref(*a, *k).unwrap() != *v {
                panic!("BAD_VALUE")
            }
        }
    }
}

fn build_one(addr: &[Address], accs: &[DbAccount]) -> HashMap<HashedAddressCell, U256, SimpleBuildHasher> {
    let mut hm: HashMap<HashedAddressCell, U256, SimpleBuildHasher> = HashMap::with_hasher(SimpleBuildHasher::default());

    for (a, addr) in addr.iter().enumerate() {
        let acc = &accs[a];
        for (k, v) in acc.storage.iter() {
            let addrcell: HashedAddressCell = HashedAddressCell(*addr, *k);
            hm.insert(addrcell, *v);
        }
    }
    hm
}

fn build_many(addr: &[Address], accs: &[DbAccount]) -> HashMap<Address, HashMap<U256, U256>> {
    let mut hm: HashMap<Address, HashMap<U256, U256>> = HashMap::new();

    for (a, addr) in addr.iter().enumerate() {
        let acc = &accs[a];
        let e = hm.entry(*addr).or_default();
        for (k, v) in acc.storage.iter() {
            e.insert(*k, *v);
        }
    }
    hm
}

fn test_build_many(addr: &[Address], accs: &[DbAccount]) {
    build_many(addr, accs);
}

fn test_read_many(addr: &[Address], accs: &[DbAccount], hm: &HashMap<Address, HashMap<U256, U256>>) {
    for (a, addr) in addr.iter().enumerate() {
        let acc = &accs[a];
        match hm.get(addr) {
            Some(s) => {
                for (k, v) in acc.storage.iter() {
                    match s.get(k) {
                        Some(cv) => {
                            if *cv != *v {
                                panic!("NE")
                            }
                        }
                        _ => panic!("NFC"),
                    }
                }
            }
            _ => {
                panic!("NF")
            }
        }
    }
}

fn test_build_one(addr: &[Address], accs: &[DbAccount]) {
    build_one(addr, accs);
}

fn test_read_one(addr: &[Address], accs: &[DbAccount], hm: &HashMap<HashedAddressCell, U256, SimpleBuildHasher>) {
    for (a, addr) in addr.iter().enumerate() {
        let acc = &accs[a];
        for (k, v) in acc.storage.iter() {
            let ac = HashedAddressCell(*addr, *k);
            match hm.get(&ac) {
                Some(cv) => {
                    if *cv != *v {
                        panic!("NE")
                    }
                }
                _ => {
                    panic!("NFC")
                }
            }
        }
    }
}

fn test_hash_speed() {
    let addr = Address::random();
    for _ in 0..N {
        let mut hasher = DefaultHasher::new();
        addr.hash(&mut hasher);
    }
}

fn test_hash_fx_speed() {
    let addr = Address::random();
    for _ in 0..N {
        let mut hasher = SimpleHasher::new();
        addr.hash(&mut hasher);
    }
}

fn test_hashedaddr_speed() {
    let addr = HashedAddress::from(Address::random());
    for _ in 0..N {
        let mut hasher = SimpleHasher::new();
        addr.hash(&mut hasher);
    }
}

fn test_hashedaddrcell_speed() {
    let addrcell = HashedAddressCell(Address::random(), U256::from(0x1234567));
    for _ in 0..N {
        let mut hasher = SimpleHasher::new();
        addrcell.hash(&mut hasher);
    }
}

fn test_hashset_speed() {
    let mut addrmap = HashSet::new();
    for _ in 0..N {
        let addr = Address::random();
        addrmap.insert(addr);
    }
}

fn test_hashmap_speed() {
    let mut addrmap = HashMap::new();
    for _ in 0..N {
        let addr = Address::random();
        addrmap.insert(addr, true);
    }
}

fn test_hashset_fx_speed() {
    let mut addrmap = HashSet::with_hasher(SimpleBuildHasher::default());
    for _ in 0..N {
        let addr = Address::random();
        addrmap.insert(addr);
    }
}

fn test_hashset_hashedaddress_speed() {
    let mut addrmap: HashSet<HashedAddress, SimpleBuildHasher> = HashSet::with_hasher(SimpleBuildHasher::default());
    for _ in 0..N {
        let addr = Address::random();
        let ha: HashedAddress = addr.into();
        addrmap.insert(ha);
    }
}

fn benchmark_test_group_hashmap(c: &mut Criterion) {
    let addr: Vec<Address> = (0..N_ACC).map(|_| Address::random()).collect();
    let accs = generate_accounts(N_ACC, N_MEM);

    let one_hm = build_one(&addr, &accs);
    let many_hm = build_many(&addr, &accs);

    let mut cache_db = CacheDB::new(EmptyDB::new());
    let mut loom_db = LoomInMemoryDB::default();

    fill_cache_db(&mut cache_db, &addr, &accs);
    fill_loom_db(&mut loom_db, &addr, &accs);

    let mut group = c.benchmark_group("hashmap_speed");
    group.sample_size(10);
    group.bench_function("test_insert_cache_db", |b| b.iter(|| test_insert_cache_db(&addr, &accs)));
    group.bench_function("test_insert_loom_db", |b| b.iter(|| test_insert_loom_db(&addr, &accs)));
    group.bench_function("test_read_cache_db", |b| b.iter(|| test_read_cache_db(&cache_db, &addr, &accs)));
    group.bench_function("test_read_loom_db", |b| b.iter(|| test_read_loom_db(&loom_db, &addr, &accs)));
    group.bench_function("test_insert_one_hashmap", |b| b.iter(|| test_build_one(&addr, &accs)));
    group.bench_function("test_insert_many_hashmap", |b| b.iter(|| test_build_many(&addr, &accs)));
    group.bench_function("test_read_one_hashmap", |b| b.iter(|| test_read_one(&addr, &accs, &one_hm)));
    group.bench_function("test_read_many_hashmap", |b| b.iter(|| test_read_many(&addr, &accs, &many_hm)));
    group.finish();
}

fn benchmark_test_group_hasher(c: &mut Criterion) {
    let mut group = c.benchmark_group("hasher_speed");
    group.bench_function("test_hash_speed", |b| b.iter(test_hash_speed));
    group.bench_function("test_hash_fx_speed", |b| b.iter(test_hash_fx_speed));
    group.bench_function("test_hash_hashedaddr_speed", |b| b.iter(test_hashedaddr_speed));
    group.bench_function("test_hash_hashedaddrcell_speed", |b| b.iter(test_hashedaddrcell_speed));
    group.bench_function("test_hashset_speed", |b| b.iter(test_hashset_speed));
    group.bench_function("test_hashset_fx_speed", |b| b.iter(test_hashset_fx_speed));
    group.bench_function("test_hashset_hashedaddress_speed", |b| b.iter(test_hashset_hashedaddress_speed));
    group.bench_function("test_hashmap_speed", |b| b.iter(test_hashmap_speed));
    group.finish();
}

criterion_group!(benches, benchmark_test_group_hashmap, benchmark_test_group_hasher);
criterion_main!(benches);
