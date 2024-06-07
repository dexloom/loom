use alloy_primitives::{Address, keccak256, U256};
use eyre::{eyre, Result};
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

pub fn try_read_hashmap_cell(db: &InMemoryDB, account: &Address, offset: &U256, index: &U256) -> Result<U256> {
    match db.accounts.get(account) {
        Some(account) => {
            let mut buf = offset.to_be_bytes::<32>().to_vec();
            buf.append(&mut index.to_be_bytes::<32>().to_vec());

            let cell: U256 = keccak256(buf.as_slice()).try_into()?;
            let value: Option<&U256> = account.storage.get(&cell);

            Ok(value.map(|x| *x).unwrap_or_default())
        }
        None => {
            Err(eyre!("NO_ACCOUNT"))
        }
    }
}

