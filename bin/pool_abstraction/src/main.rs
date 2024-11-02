#[allow(dead_code, unused_variables)]
use std::collections::HashMap;

trait DataBase {
    fn read(&self) -> u32;
}

struct DataBaseImpl {
    id: u32,
}

impl DataBaseImpl {
    fn new(id: u32) -> Self {
        Self { id }
    }
}

impl DataBase for DataBaseImpl {
    fn read(&self) -> u32 {
        self.id
    }
}

trait CleanPool {
    fn name(&self) -> String;
    fn calc_with_db(&self, db: Box<dyn DataBase>) -> u32;
}

trait Pool<DB>
where
    DB: DataBase + Sized + 'static,
{
    fn name(&self) -> String;
    fn calc(&self, db: DB) -> u32;

    fn calc_with_db(&self, db: Box<dyn DataBase>) -> u32;
}

struct PoolImpl1<DB> {
    db: DB,
}
impl<DB> Pool<DB> for PoolImpl1<DB>
where
    DB: DataBase + 'static,
{
    fn name(&self) -> String {
        "P1".to_string()
    }

    fn calc(&self, db: DB) -> u32 {
        self.db.read()
    }

    fn calc_with_db(&self, db: Box<dyn DataBase>) -> u32 {
        db.read()
    }
}

#[derive(Clone)]
struct CleanPoolImpl1 {}

impl CleanPool for CleanPoolImpl1 {
    fn name(&self) -> String {
        "CLEANPOOL1".to_string()
    }

    fn calc_with_db(&self, db: Box<dyn DataBase>) -> u32 {
        db.read()
    }
}

#[derive(Default)]
struct CleanMarket {
    map: HashMap<String, Box<dyn CleanPool>>,
}

impl CleanMarket {
    fn add(&mut self, pool: Box<dyn CleanPool>) {
        self.map.insert(pool.name(), pool);
    }

    fn get(&self, name: &String) -> Option<&Box<dyn CleanPool>> {
        self.map.get(name)
    }
}

struct Market<DB>
where
    DB: DataBase + Sized + 'static,
{
    map: HashMap<String, Box<dyn Pool<DB> + 'static>>,
}

impl<DB> Market<DB>
where
    DB: DataBase + 'static,
{
    fn add(&mut self, pool: Box<dyn Pool<DB>>) {
        self.map.insert(pool.name(), pool);
    }
}

fn main() {
    let db = Box::new(DataBaseImpl { id: 1 });

    let empty_pool_0 = CleanPoolImpl1 {};

    let mut clean_market = CleanMarket::default();

    clean_market.add(Box::new(empty_pool_0));

    let pool = clean_market.get(&"CLEANPOOL1".to_string()).unwrap();

    pool.calc_with_db(db);
}
