use crate::tick_provider::TickProvider;
use crate::U256_1;
use crate::{bit_math, error::UniswapV3MathError};
use alloy::primitives::{Address, BlockNumber, U256};
use alloy::providers::Provider;
use alloy::sol;
use std::{collections::HashMap, sync::Arc};

sol! {
    #[sol(rpc)]
    interface IUniswapV3Pool {
        function tick_bitmap(int16) external returns (int16);
    }
}

//Flips the initialized state for a given tick from false to true, or vice versa
pub fn flip_tick(tick_bitmap: &mut HashMap<i16, U256>, tick: i32, tick_spacing: i32) -> Result<(), UniswapV3MathError> {
    if (tick % tick_spacing) != 0 {
        return Err(UniswapV3MathError::TickSpacingError);
    }

    let (word_pos, bit_pos) = position(tick / tick_spacing);
    let mask = U256_1 << bit_pos;
    let word = *tick_bitmap.get(&word_pos).unwrap_or(&U256::ZERO);
    tick_bitmap.insert(word_pos, word ^ mask);
    Ok(())
}

//Returns the next initialized tick contained in the same word (or adjacent word) as the tick that is either
//to the left (less than or equal to) or right (greater than) of the given tick
pub fn next_initialized_tick_within_one_word<T: TickProvider>(
    tick_provider: &T,
    tick: i32,
    tick_spacing: i32,
    lte: bool,
) -> Result<(i32, bool), UniswapV3MathError> {
    let compressed = if tick < 0 && tick % tick_spacing != 0 { (tick / tick_spacing) - 1 } else { tick / tick_spacing };

    if lte {
        let (word_pos, bit_pos) = position(compressed);

        let mask = (U256_1 << bit_pos) - U256_1 + (U256_1 << bit_pos);

        let masked = tick_provider.get_tick(word_pos).unwrap_or(U256::ZERO) & mask;

        let initialized = !masked.is_zero();

        let next = if initialized {
            (compressed - (bit_pos.overflowing_sub(bit_math::most_significant_bit(masked)?).0) as i32) * tick_spacing
        } else {
            (compressed - bit_pos as i32) * tick_spacing
        };

        Ok((next, initialized))
    } else {
        let (word_pos, bit_pos) = position(compressed + 1);

        let mask = !((U256_1 << bit_pos) - U256_1);

        let masked = tick_provider.get_tick(word_pos).unwrap_or(U256::ZERO) & mask;

        let initialized = !masked.is_zero();

        let next = if initialized {
            (compressed + 1 + (bit_math::least_significant_bit(masked)?.overflowing_sub(bit_pos).0) as i32) * tick_spacing
        } else {
            (compressed + 1 + ((0xFF - bit_pos) as i32)) * tick_spacing
        };

        Ok((next, initialized))
    }
}

//Returns next and initialized. This function calls the node to get the word at the word_pos.
//current_word is the current word in the TickBitmap of the pool based on `tick`. TickBitmap[word_pos] = current_word
//Where word_pos is the 256 bit offset of the ticks word_pos.. word_pos := tick >> 8
pub async fn next_initialized_tick_within_one_word_from_provider<P: Provider>(
    tick: i32,
    tick_spacing: i32,
    lte: bool,
    pool_address: Address,
    block_number: Option<BlockNumber>,
    provider: Arc<P>,
) -> Result<(i32, bool), UniswapV3MathError> {
    let compressed = if tick < 0 && tick % tick_spacing != 0 { (tick / tick_spacing) - 1 } else { tick / tick_spacing };

    if lte {
        let (word_pos, bit_pos) = position(compressed);
        let mask = (U256_1 << bit_pos) - U256_1 + (U256_1 << bit_pos);

        let word = if let Some(block_number) = block_number {
            match IUniswapV3Pool::new(pool_address, provider).tick_bitmap(word_pos).block(block_number.into()).call().await {
                Ok(word) => U256::from(word._0),
                Err(err) => return Err(UniswapV3MathError::MiddlewareError(err.to_string())),
            }
        } else {
            match IUniswapV3Pool::new(pool_address, provider).tick_bitmap(word_pos).call().await {
                Ok(word) => U256::from(word._0),
                Err(err) => return Err(UniswapV3MathError::MiddlewareError(err.to_string())),
            }
        };

        let masked = word & mask;

        let initialized = !masked.is_zero();

        let next = if initialized {
            (compressed - (bit_pos.overflowing_sub(bit_math::most_significant_bit(masked)?).0) as i32) * tick_spacing
        } else {
            (compressed - bit_pos as i32) * tick_spacing
        };

        Ok((next, initialized))
    } else {
        let (word_pos, bit_pos) = position(compressed + 1);
        let mask = !((U256_1 << bit_pos) - U256_1);

        let word = if let Some(block_number) = block_number {
            match IUniswapV3Pool::new(pool_address, provider).tick_bitmap(word_pos).block(block_number.into()).call().await {
                Ok(word) => U256::from(word._0),
                Err(err) => return Err(UniswapV3MathError::MiddlewareError(err.to_string())),
            }
        } else {
            match IUniswapV3Pool::new(pool_address, provider).tick_bitmap(word_pos).call().await {
                Ok(word) => U256::from(word._0),
                Err(err) => return Err(UniswapV3MathError::MiddlewareError(err.to_string())),
            }
        };

        let masked = word & mask;
        let initialized = !masked.is_zero();

        let next = if initialized {
            (compressed + 1 + (bit_math::least_significant_bit(masked)?.overflowing_sub(bit_pos).0) as i32) * tick_spacing
        } else {
            (compressed + 1 + ((0xFF - bit_pos) as i32)) * tick_spacing
        };

        Ok((next, initialized))
    }
}

//Computes the position in the mapping where the initialized bit for a tick lives
pub fn position(tick: i32) -> (i16, u8) {
    ((tick >> 8) as i16, (tick % 256) as u8)
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, vec};

    use super::{flip_tick, next_initialized_tick_within_one_word};
    use crate::tick_provider::TickProvider;
    use alloy::primitives::U256;

    pub struct TickProviderHashMap {
        tick_bitmap: HashMap<i16, U256>,
    }
    impl TickProviderHashMap {
        pub fn new(tick_bitmap: HashMap<i16, U256>) -> Self {
            Self { tick_bitmap }
        }
        pub fn mut_ref(&mut self) -> &mut HashMap<i16, U256> {
            &mut self.tick_bitmap
        }
        pub fn clear(&mut self) {
            self.tick_bitmap.clear();
        }
    }
    impl TickProvider for TickProviderHashMap {
        fn get_tick(&self, word_pos: i16) -> eyre::Result<U256> {
            let val = self.tick_bitmap.get(&word_pos).cloned();
            Ok(val.unwrap_or(U256::ZERO))
        }
    }

    pub fn init_test_ticks() -> eyre::Result<TickProviderHashMap> {
        let test_ticks = vec![-200, -55, -4, 70, 78, 84, 139, 240, 535];
        let mut tick_bitmap: HashMap<i16, U256> = HashMap::new();
        for tick in test_ticks {
            flip_tick(&mut tick_bitmap, tick, 1)?;
        }
        Ok(TickProviderHashMap::new(tick_bitmap))
    }

    pub fn initialized<T: TickProvider>(tick: i32, tick_provider: &T) -> eyre::Result<bool> {
        let (next, initialized) = next_initialized_tick_within_one_word(tick_provider, tick, 1, true)?;
        if next == tick {
            Ok(initialized)
        } else {
            Ok(false)
        }
    }

    #[test]
    pub fn test_next_initialized_tick_within_one_word_lte_false() -> eyre::Result<()> {
        let mut tick_bitmap = init_test_ticks()?;
        //returns tick to right if at initialized tick
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 78, 1, false)?;
        assert_eq!(next, 84);
        assert_eq!(initialized, true);
        tick_bitmap = init_test_ticks()?;
        // //returns the tick directly to the right
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 77, 1, false)?;

        assert_eq!(next, 78);
        assert_eq!(initialized, true);
        tick_bitmap = init_test_ticks()?;
        // //returns the tick directly to the right
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, -56, 1, false)?;

        assert_eq!(next, -55);
        assert_eq!(initialized, true);
        tick_bitmap = init_test_ticks()?;
        //returns the next words initialized tick if on the right boundary
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 255, 1, false)?;

        assert_eq!(next, 511);
        assert_eq!(initialized, false);
        tick_bitmap = init_test_ticks()?;
        //returns the next words initialized tick if on the right boundary
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, -257, 1, false)?;

        assert_eq!(next, -200);
        assert_eq!(initialized, true);
        tick_bitmap = init_test_ticks()?;
        //returns the next initialized tick from the next word
        flip_tick(&mut tick_bitmap.mut_ref(), 340, 1)?;
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 328, 1, false)?;

        assert_eq!(next, 340);
        assert_eq!(initialized, true);
        tick_bitmap = init_test_ticks()?;
        //does not exceed boundary
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 508, 1, false)?;

        assert_eq!(next, 511);
        assert_eq!(initialized, false);
        tick_bitmap = init_test_ticks()?;
        //skips entire word
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 255, 1, false)?;

        assert_eq!(next, 511);
        assert_eq!(initialized, false);
        tick_bitmap = init_test_ticks()?;
        //skips half word
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 383, 1, false)?;

        assert_eq!(next, 511);
        assert_eq!(initialized, false);
        Ok(())
    }

    #[test]
    pub fn test_next_initialized_tick_within_one_word_lte_true() -> eyre::Result<()> {
        let mut tick_bitmap = init_test_ticks()?;
        //returns same tick if initialized
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 78, 1, true)?;
        assert_eq!(next, 78);
        assert_eq!(initialized, true);
        tick_bitmap = init_test_ticks()?;
        //returns tick directly to the left of input tick if not initialized
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 79, 1, true)?;

        assert_eq!(next, 78);
        assert_eq!(initialized, true);
        tick_bitmap = init_test_ticks()?;
        //will not exceed the word boundary
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 258, 1, true)?;

        assert_eq!(next, 256);
        assert_eq!(initialized, false);
        tick_bitmap = init_test_ticks()?;
        //at the word boundary
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 256, 1, true)?;

        assert_eq!(next, 256);
        assert_eq!(initialized, false);
        tick_bitmap = init_test_ticks()?;
        //word boundary less 1 (next initialized tick in next word)',
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 72, 1, true)?;

        assert_eq!(next, 70);
        assert_eq!(initialized, true);
        tick_bitmap = init_test_ticks()?;
        //word boundary
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, -257, 1, true)?;

        assert_eq!(next, -512);
        assert_eq!(initialized, false);
        tick_bitmap = init_test_ticks()?;
        //entire empty word
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 1023, 1, true)?;

        assert_eq!(next, 768);
        assert_eq!(initialized, false);
        tick_bitmap = init_test_ticks()?;
        //halfway through empty word
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 900, 1, true)?;

        assert_eq!(next, 768);
        assert_eq!(initialized, false);
        tick_bitmap = init_test_ticks()?;
        //boundary is initialized
        flip_tick(&mut tick_bitmap.mut_ref(), 329, 1)?;
        let (next, initialized) = next_initialized_tick_within_one_word(&tick_bitmap, 456, 1, true)?;

        assert_eq!(next, 329);
        assert_eq!(initialized, true);
        Ok(())
    }

    #[test]
    pub fn test_initialized() -> eyre::Result<()> {
        //is false at first
        let mut tick_bitmap = TickProviderHashMap::new(HashMap::new());
        let is_initialized = initialized(1, &tick_bitmap)?;

        assert_eq!(is_initialized, false);
        //is flipped by #flipTick
        flip_tick(&mut tick_bitmap.mut_ref(), 1, 1)?;
        let is_initialized: bool = initialized(1, &tick_bitmap)?;
        assert_eq!(is_initialized, true);

        //is flipped back by #flipTick
        tick_bitmap.clear();
        flip_tick(&mut tick_bitmap.mut_ref(), 1, 1)?;
        flip_tick(&mut tick_bitmap.mut_ref(), 1, 1)?;
        let is_initialized = initialized(1, &tick_bitmap)?;
        assert_eq!(is_initialized, false);

        //is not changed by another flip to a different tick
        tick_bitmap.clear();
        flip_tick(&mut tick_bitmap.mut_ref(), 2, 1)?;
        let is_initialized = initialized(1, &tick_bitmap)?;
        assert_eq!(is_initialized, false);

        //is not changed by another flip to a different tick on another word
        tick_bitmap.clear();
        flip_tick(&mut tick_bitmap.mut_ref(), 1 + 256, 1)?;
        let is_initialized = initialized(257, &tick_bitmap)?;
        assert_eq!(is_initialized, true);
        let is_initialized = initialized(1, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        Ok(())
    }

    #[test]
    pub fn test_flip_tick() -> eyre::Result<()> {
        //flips only the specified tick
        let mut tick_bitmap = TickProviderHashMap::new(HashMap::new());
        flip_tick(tick_bitmap.mut_ref(), -230, 1)?;
        let is_initialized = initialized(-230, &tick_bitmap)?;
        assert_eq!(is_initialized, true);
        let is_initialized = initialized(-231, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        let is_initialized = initialized(-229, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        let is_initialized = initialized(-230 + 256, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        let is_initialized = initialized(-230 - 256, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        flip_tick(tick_bitmap.mut_ref(), -230, 1)?;
        let is_initialized = initialized(-230, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        let is_initialized = initialized(-231, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        let is_initialized = initialized(-229, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        let is_initialized = initialized(-230 + 256, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        let is_initialized = initialized(-230 - 256, &tick_bitmap)?;
        assert_eq!(is_initialized, false);
        //reverts only itself
        tick_bitmap.clear();
        flip_tick(tick_bitmap.mut_ref(), -230, 1)?;
        flip_tick(tick_bitmap.mut_ref(), -259, 1)?;
        flip_tick(tick_bitmap.mut_ref(), -229, 1)?;
        flip_tick(tick_bitmap.mut_ref(), 500, 1)?;
        flip_tick(tick_bitmap.mut_ref(), -259, 1)?;
        flip_tick(tick_bitmap.mut_ref(), -229, 1)?;
        flip_tick(tick_bitmap.mut_ref(), -259, 1)?;
        let is_initialized = initialized(-259, &tick_bitmap)?;
        assert_eq!(is_initialized, true);
        let is_initialized = initialized(-229, &tick_bitmap)?;
        assert_eq!(is_initialized, false);

        Ok(())
    }
}
