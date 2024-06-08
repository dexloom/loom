use alloy_primitives::{Address, keccak256, U256};
use eyre::{eyre, Result};
use log::{debug, trace};
use revm::InMemoryDB;

pub fn try_read_cell(db: &InMemoryDB, account: &Address, cell: &U256) -> Result<U256> {
    match db.accounts.get(account) {
        Some(account) => {
            match account.storage.get(cell) {
                Some(data) => {
                    Ok(*data)
                }
                None => {
                    Err(eyre!("NO_CELL"))
                }
            }
        }
        None => {
            Err(eyre!("NO_ACCOUNT"))
        }
    }
}

pub fn try_read_hashmap_cell(db: &InMemoryDB, account: &Address, hashmap_offset: &U256, item: &U256) -> Result<U256> {
    match db.accounts.get(account) {
        Some(account) => {
            let mut buf = item.to_be_bytes::<32>().to_vec();
            buf.append(&mut hashmap_offset.to_be_bytes::<32>().to_vec());
            trace!("try_read_hashmap_cell {buf:?}");

            let cell: U256 = keccak256(buf.as_slice()).try_into()?;
            let value: Option<&U256> = account.storage.get(&cell);

            Ok(value.map(|x| *x).unwrap_or_default())
        }
        None => {
            Err(eyre!("NO_ACCOUNT"))
        }
    }
}

