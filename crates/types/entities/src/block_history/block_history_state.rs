use crate::market_state::MarketStateConfig;
use crate::BlockHistoryEntry;
use loom_evm_db::{DatabaseLoomExt, LoomDB};
use tracing::{error, trace};

pub trait BlockHistoryState {
    fn apply_update(self, block_history_entry: &BlockHistoryEntry, market_state_config: &MarketStateConfig) -> Self;
}

impl BlockHistoryState for LoomDB {
    fn apply_update(self, block_history_entry: &BlockHistoryEntry, market_state_config: &MarketStateConfig) -> Self {
        let mut db = self;
        if let Some(state_update) = &block_history_entry.state_update {
            for state_diff in state_update.iter() {
                for (address, account_state) in state_diff.iter() {
                    if let Some(balance) = account_state.balance {
                        if db.is_rw_ro_account(address) {
                            match db.load_ro_rw_account(*address) {
                                Ok(x) => {
                                    x.info.balance = balance;
                                    trace!("Balance updated {:#20x} {}", address, balance);
                                }
                                _ => {
                                    trace!("Balance updated for {:#20x} not found", address);
                                }
                            };
                        }
                    }

                    if let Some(nonce) = account_state.nonce {
                        if db.is_account(address) {
                            match db.load_cached_account(*address) {
                                Ok(x) => {
                                    x.info.nonce = nonce;
                                    trace!("Nonce updated {:#20x} {}", address, nonce);
                                }
                                _ => {
                                    trace!("Nonce updated for {:#20x} not found", address);
                                }
                            };
                        }
                    }

                    for (slot, value) in account_state.storage.iter() {
                        if market_state_config.is_force_insert(address) {
                            trace!("Force slot updated {:#20x} {} {}", address, slot, value);
                            if let Err(e) = db.insert_account_storage(*address, (*slot).into(), (*value).into()) {
                                error!("{}", e)
                            }
                        } else if db.is_slot(address, &(*slot).into()) {
                            trace!("Slot updated {:#20x} {} {}", address, slot, value);
                            if let Err(e) = db.insert_account_storage(*address, (*slot).into(), (*value).into()) {
                                error!("{}", e)
                            }
                        }
                    }
                }
            }
        }

        db
    }
}
