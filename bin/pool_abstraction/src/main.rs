use eyre::{eyre, ErrReport};
use std::any::Any;
#[allow(dead_code, unused_variables)]
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;

trait DataBase {
    type Error;
    fn read(&self) -> eyre::Result<u32, ErrReport>;
}

#[derive(Clone)]
struct DataBaseImpl {
    id: u32,
}

impl DataBaseImpl {
    fn new(id: u32) -> Self {
        Self { id }
    }
}

impl DataBase for DataBaseImpl {
    type Error = ErrReport;
    fn read(&self) -> Result<u32, ErrReport> {
        Ok(self.id)
    }
}

trait CleanPool {
    fn name(&self) -> String;
    fn calc_with_db(&self, db: &dyn DataBase<Error = &dyn Display>) -> Result<u32, ErrReport>;
}

#[derive(Clone)]
struct CleanPoolImpl1 {}

impl CleanPool for CleanPoolImpl1 {
    fn name(&self) -> String {
        "CLEANPOOL1".to_string()
    }

    fn calc_with_db(&self, db: &dyn DataBase<Error = &dyn Display>) -> Result<u32, ErrReport> {
        db.read().map_err(|e| eyre!(e))
    }
}

trait Pool<DB>
where
    DB: DataBase + Sized + 'static,
{
    fn name(&self) -> String;
    fn calc(&self, db: DB) -> u32;

    fn calc_with_db(&self, db: &DB) -> Result<u32, ErrReport>;
}

struct PoolImpl1<DB> {
    db: DB,
}

impl<DB> PoolImpl1<DB>
where
    DB: DataBase,
{
    pub fn new(db: DB) -> Self {
        Self { db }
    }
}

impl<DB> Pool<DB> for PoolImpl1<DB>
where
    DB: DataBase + 'static,
{
    fn name(&self) -> String {
        "P1".to_string()
    }

    fn calc(&self, db: DB) -> u32 {
        self.db.read().unwrap()
    }

    fn calc_with_db(&self, db: &DB) -> Result<u32, ErrReport> {
        db.read().map_err(|e| eyre!(e))
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
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }
    fn add(&mut self, pool: Box<dyn Pool<DB>>) {
        self.map.insert(pool.name(), pool);
    }

    fn get(&self, name: &String) -> Option<&Box<dyn Pool<DB>>> {
        self.map.get(name)
    }
}

fn main() {
    let db = DataBaseImpl { id: 1 };

    let empty_pool_0 = PoolImpl1::new(db.clone());

    let mut market = Market::<DataBaseImpl>::new();

    market.add(Box::new(empty_pool_0));

    let pool = market.get(&"CLEANPOOL1".to_string()).unwrap();

    pool.calc_with_db(&db);
}