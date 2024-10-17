use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use alloy_primitives::{Address, U256};
use alloy_primitives::aliases::I24;
use alloy_primitives::ruint::__private::ruint_macro::uint;
use tracing::{error, info};
use uniswap_v3_sdk::entities::{Tick, TickIndex};
use uniswap_v3_sdk::error::Error;
use uniswap_v3_sdk::prelude::{BitMath, TickDataProvider};
use loom_revm_db::LoomInMemoryDB;
use uniswap_v3_math::full_math::TWO;
use crate::db_reader::UniswapV3DBReader;
use crate::UniswapV3Pool;

#[derive(Clone)]
pub struct LoomTickDataProvider {
    state_db: Arc<LoomInMemoryDB>,
    pool_address: Address,
    ticks: RefCell<HashMap<I24, Tick<I24>>>,
}

impl LoomTickDataProvider {
    pub fn new(state_db: Arc<LoomInMemoryDB>, pool_address: Address, ticks: RefCell<HashMap<I24, Tick<I24>>>) -> Self {
        Self {
            state_db,
            pool_address,
            ticks,
        }
    }
}

impl TickDataProvider for LoomTickDataProvider {
    type Index = I24;

    fn get_tick(&self, tick: Self::Index) -> Result<&Tick<Self::Index>, Error> {
        //println!("get_tick: tick={}", tick);
        let tick_info = match UniswapV3DBReader::ticks(&self.state_db, self.pool_address, tick) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to fetch tick info from db for tick={}, err={:?}", tick, e);
                return Err(Error::InvalidTick(tick))
            },
        };

        let tick_entity = Tick::new(tick, tick_info.liquidityGross, tick_info.liquidityNet);
        self.ticks.borrow_mut().insert(tick, tick_entity);

        let ticks_ref = unsafe { self.ticks.as_ptr().as_ref() }.unwrap();
        match ticks_ref.get(&tick) {
            Some(v) => Ok(v),
            None => Err(Error::InvalidTick(tick)),
        }
    }

    fn next_initialized_tick_within_one_word(&self, tick: Self::Index, lte: bool, tick_spacing: Self::Index) -> Result<(Self::Index, bool), Error> {
        // CODE ADAPTED FROM src/extensions/tick_map.rs
        let compressed = tick.compress(tick_spacing);
        if lte {
            let (word_pos, bit_pos) = compressed.position();
            // all the 1s at or to the right of the current `bit_pos`
            // (2 << bitPos) may overflow but fine since 2 << 255 = 0
            let mask = (TWO << bit_pos) - U256::from(1);

            let word = match UniswapV3DBReader::tick_bitmap(&self.state_db, self.pool_address, word_pos.as_i32()) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to fetch tick bitmap from db for tick={}, err={:?}", tick, e);
                    return Err(Error::InvalidTick(tick))
                },
            };

            let masked = word & mask;
            let initialized = masked != U256::ZERO;
            let bit_pos = if initialized {
                let msb = masked.most_significant_bit() as u8;
                (bit_pos - msb) as i32
            } else {
                bit_pos as i32
            };
            let next = (compressed - Self::Index::try_from(bit_pos).unwrap()) * tick_spacing;
            Ok((next, initialized))
        } else {
            let (word_pos, bit_pos) = compressed.position();
            // all the 1s at or to the left of the `bit_pos`
            let mask = U256::ZERO - (U256::from(1) << bit_pos);

            let word = match UniswapV3DBReader::tick_bitmap(&self.state_db, self.pool_address, word_pos.as_i32()) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to fetch tick bitmap from db for tick={}, err={:?}", tick, e);
                    return Err(Error::InvalidTick(tick))
                },
            };
            let masked = word & mask;
            let initialized = masked != U256::ZERO;
            let bit_pos = if initialized {
                let lsb = masked.least_significant_bit() as u8;
                (lsb - bit_pos) as i32
            } else {
                (255 - bit_pos) as i32
            };
            let next = (compressed + Self::Index::try_from(bit_pos).unwrap()) * tick_spacing;


            let tick_info = match UniswapV3DBReader::ticks(&self.state_db, self.pool_address, next) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to fetch tick info from db for tick={}, err={:?}", tick, e);
                    return Err(Error::InvalidTick(tick))
                },
            };
            //println!("tick_info: {:?}", tick_info);
            //println!("next_initialized_tick_within_one_word: tick={}, lte={}, tick_spacing={}, next={}, initialized={}", tick, lte, tick_spacing, next, initialized);
            if !tick_info.initialized {
                return Ok((Self::Index::ZERO, false))
            }


            Ok((next, initialized))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_loom_tick_data_provider() {
        let state_db = Arc::new(LoomInMemoryDB::default());
        let pool_address = Address::default();
        let ticks = RefCell::new(HashMap::new());
        let mut provider = LoomTickDataProvider::new(state_db, pool_address, ticks);
        let tick = provider.get_tick(I24::ZERO);

    }
}