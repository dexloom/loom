use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use alloy::primitives::{Address, Bytes, U256};
use criterion::{Criterion, criterion_group, criterion_main};
use rand::{Rng, thread_rng};
use revm::db::{AccountState as DbAccountState, CacheDB, DbAccount, EmptyDB};
use revm::primitives::{AccountInfo, Bytecode, KECCAK_EMPTY};

use loom_revm::fast_hasher::{HashedAddress, HashedAddressCell, SimpleBuildHasher, SimpleHasher};
use loom_revm::fast_inmemorydb::FastInMemoryDb;

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

fn test_insert(addr: &Vec<Address>, accs: &Vec<DbAccount>) {
    let mut db0 = CacheDB::new(EmptyDB::new());
    for a in 0..addr.len() {
        db0.accounts.insert(addr[a], accs[a].clone());
    }
}

fn build_one(addr: &Vec<Address>, accs: &Vec<DbAccount>) -> HashMap<HashedAddressCell, U256, SimpleBuildHasher> {
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


fn build_many(addr: &Vec<Address>, accs: &Vec<DbAccount>) -> HashMap<Address, HashMap<U256, U256>> {
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

fn test_build_many(addr: &Vec<Address>, accs: &Vec<DbAccount>) {
    build_many(addr, accs);
}

fn test_read_many(addr: &Vec<Address>, accs: &Vec<DbAccount>, hm: &HashMap<Address, HashMap<U256, U256>>) {
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
                        _ => panic!("NFC")
                    }
                }
            }
            _ => {
                panic!("NF")
            }
        }
    }
}

fn test_build_one(addr: &Vec<Address>, accs: &Vec<DbAccount>) {
    build_one(addr, accs);
}

fn test_read_one(addr: &Vec<Address>, accs: &Vec<DbAccount>, hm: &HashMap<HashedAddressCell, U256, SimpleBuildHasher>) {
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


fn test_hash_speed() {
    let addr = Address::random();
    for i in 0..N {
        let mut hasher = DefaultHasher::new();
        let hash = addr.hash(&mut hasher);
    }
}

fn test_hash_fx_speed() {
    let addr = Address::random();
    for i in 0..N {
        let mut hasher = SimpleHasher::new();
        let hash = addr.hash(&mut hasher);
    }
}

fn test_hashedaddr_speed() {
    let addr = HashedAddress::from(Address::random());
    for i in 0..N {
        let mut hasher = SimpleHasher::new();
        let hash = addr.hash(&mut hasher);
    }
}

fn test_hashedaddrcell_speed() {
    let addrcell = HashedAddressCell(Address::random(), U256::from(0x1234567));
    for i in 0..N {
        let mut hasher = SimpleHasher::new();
        let hash = addrcell.hash(&mut hasher);
    }
}


fn test_hashset_speed() {
    let mut addrmap = HashSet::new();
    for i in 0..N {
        let addr = Address::random();
        addrmap.insert(addr);
    }
}

fn test_hashmap_speed() {
    let mut addrmap = HashMap::new();
    for i in 0..N {
        let addr = Address::random();
        addrmap.insert(addr, true);
    }
}

fn test_hashset_fx_speed() {
    let mut addrmap = HashSet::with_hasher(SimpleBuildHasher::default());
    for i in 0..N {
        let addr = Address::random();
        addrmap.insert(addr);
    }
}

fn test_hashset_hashedaddress_speed() {
    let mut addrmap: HashSet<HashedAddress, SimpleBuildHasher> = HashSet::with_hasher(SimpleBuildHasher::default());
    for i in 0..N {
        let addr = Address::random();
        let ha: HashedAddress = addr.into();
        addrmap.insert(ha);
    }
}


fn benchmark_test_group_hashmap(c: &mut Criterion) {
    let addr: Vec<Address> = (0..N_ACC).map(|x| Address::random()).collect();
    let accs = generate_accounts(N_ACC, N_MEM);

    let one_hm = build_one(&addr, &accs);
    let many_hm = build_many(&addr, &accs);


    let mut group = c.benchmark_group("hashmap_speed");
    group.sample_size(10);
    group.bench_function("test_insert", |b| b.iter(|| test_insert(&addr, &accs)));
    group.bench_function("test_insert_one_hashmap", |b| b.iter(|| test_build_one(&addr, &accs)));
    group.bench_function("test_insert_many_hashmap", |b| b.iter(|| test_build_many(&addr, &accs)));
    group.bench_function("test_read_one_hashmap", |b| b.iter(|| test_read_one(&addr, &accs, &one_hm)));
    group.bench_function("test_read_many_hashmap", |b| b.iter(|| test_read_many(&addr, &accs, &many_hm)));
    group.finish();
}

fn benchmark_test_group_hasher(c: &mut Criterion) {
    let mut group = c.benchmark_group("hasher_speed");
    group.bench_function("test_hash_speed", |b| b.iter(|| test_hash_speed()));
    group.bench_function("test_hash_fx_speed", |b| b.iter(|| test_hash_fx_speed()));
    group.bench_function("test_hash_hashedaddr_speed", |b| b.iter(|| test_hashedaddr_speed()));
    group.bench_function("test_hash_hashedaddrcell_speed", |b| b.iter(|| test_hashedaddrcell_speed()));
    group.bench_function("test_hashset_speed", |b| b.iter(|| test_hashset_speed()));
    group.bench_function("test_hashset_fx_speed", |b| b.iter(|| test_hashset_fx_speed()));
    group.bench_function("test_hashset_hashedaddress_speed", |b| b.iter(|| test_hashset_hashedaddress_speed()));
    group.bench_function("test_hashmap_speed", |b| b.iter(|| test_hashmap_speed()));
    group.finish();
}


criterion_group!(benches,  benchmark_test_group_hashmap, benchmark_test_group_hasher);
criterion_main!(benches);