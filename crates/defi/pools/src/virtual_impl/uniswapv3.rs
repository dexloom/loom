use alloy::primitives::{Address, I256, U256};
use eyre::eyre;
use loom_defi_uniswap_v3_math::tick_math::{MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK};
use revm::DatabaseRef;

use crate::db_reader::UniswapV3DBReader;
use crate::virtual_impl::tick_provider::TickProviderEVMDB;
use crate::UniswapV3Pool;
use loom_types_entities::Pool;

pub struct UniswapV3PoolVirtual;

/* Unused useful constants
pub const U256_0X100000000: U256 = U256::from_limbs([4294967296, 0, 0, 0]);
pub const U256_0X10000: U256 = U256::from_limbs([65536, 0, 0, 0]);
pub const U256_0X100: U256 = U256::from_limbs([256, 0, 0, 0]);
pub const U256_255: U256 = U256::from_limbs([255, 0, 0, 0]);
pub const U256_192: U256 = U256::from_limbs([192, 0, 0, 0]);
pub const U256_191: U256 = U256::from_limbs([191, 0, 0, 0]);
pub const U256_128: U256 = U256::from_limbs([128, 0, 0, 0]);
pub const U256_64: U256 = U256::from_limbs([64, 0, 0, 0]);
pub const U256_32: U256 = U256::from_limbs([32, 0, 0, 0]);
pub const U256_16: U256 = U256::from_limbs([16, 0, 0, 0]);
pub const U256_8: U256 = U256::from_limbs([8, 0, 0, 0]);
pub const U256_4: U256 = U256::from_limbs([4, 0, 0, 0]);
pub const U256_2: U256 = U256::from_limbs([2, 0, 0, 0]);

pub const POPULATE_TICK_DATA_STEP: u64 = 100000;

pub const Q128: U256 = U256::from_limbs([0, 0, 1, 0]);
pub const Q224: U256 = U256::from_limbs([0, 0, 0, 4294967296]);

pub const U128_0X10000000000000000: u128 = 18446744073709551616;
pub const U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: U256 = U256::from_limbs([
    18446744073709551615,
    18446744073709551615,
    18446744073709551615,
    0,
]);
pub const U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: U256 =
    U256::from_limbs([18446744073709551615, 18446744073709551615, 0, 0]);

*/

// commonly used U256s
pub const U256_1: U256 = U256::from_limbs([1, 0, 0, 0]);

// Uniswap V3 specific

// Others

pub struct CurrentState {
    amount_specified_remaining: I256,
    amount_calculated: I256,
    sqrt_price_x_96: U256,
    tick: i32,
    liquidity: u128,
}

#[derive(Default)]
pub struct StepComputations {
    pub sqrt_price_start_x_96: U256,
    pub tick_next: i32,
    pub initialized: bool,
    pub sqrt_price_next_x96: U256,
    pub amount_in: U256,
    pub amount_out: U256,
    pub fee_amount: U256,
}

#[allow(dead_code)]
pub struct Tick {
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
    pub fee_growth_outside_0_x_128: U256,
    pub fee_growth_outside_1_x_128: U256,
    pub tick_cumulative_outside: U256,
    pub seconds_per_liquidity_outside_x_128: U256,
    pub seconds_outside: u32,
    pub initialized: bool,
}

impl UniswapV3PoolVirtual {
    pub fn simulate_swap_in_amount_provider<DB: DatabaseRef>(
        db: &DB,
        pool: &UniswapV3Pool,
        token_in: Address,
        amount_in: U256,
    ) -> eyre::Result<U256> {
        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        let zero_for_one = token_in == pool.get_tokens()[0];

        // Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one { MIN_SQRT_RATIO + U256_1 } else { MAX_SQRT_RATIO - U256_1 };

        let pool_address = pool.get_address();

        let slot0 = UniswapV3DBReader::slot0(&db, pool_address)?;
        let liquidity = UniswapV3DBReader::liquidity(&db, pool_address)?;
        let tick_spacing = pool.tick_spacing();
        let fee = pool.fee;

        // Initialize a mutable state struct to hold the dynamic simulated state of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: slot0.sqrtPriceX96.to(),              //Active price on the pool
            amount_calculated: I256::ZERO,                         //Amount of token_out that has been calculated
            amount_specified_remaining: I256::from_raw(amount_in), //Amount of token_in that has not been swapped
            tick: slot0.tick.as_i32(),                             //Current i24 tick of the pool
            liquidity,                                             //Current available liquidity in the tick range
        };

        let tick_provider = TickProviderEVMDB::new(db, pool_address);

        while current_state.amount_specified_remaining != I256::ZERO && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96 {
            // Initialize a new step struct to hold the dynamic state of the pool at each step
            let mut step = StepComputations {
                // Set the sqrt_price_start_x_96 to the current sqrt_price_x_96
                sqrt_price_start_x_96: current_state.sqrt_price_x_96,
                ..Default::default()
            };

            // Get the next tick from the current tick
            (step.tick_next, step.initialized) = loom_defi_uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                &tick_provider,
                current_state.tick,
                tick_spacing as i32,
                zero_for_one,
            )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 = loom_defi_uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            // Target spot price
            let swap_target_sqrt_ratio = if zero_for_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            // Compute swap step and update the current state
            (current_state.sqrt_price_x_96, step.amount_in, step.amount_out, step.fee_amount) =
                loom_defi_uniswap_v3_math::swap_math::compute_swap_step(
                    current_state.sqrt_price_x_96,
                    swap_target_sqrt_ratio,
                    current_state.liquidity,
                    current_state.amount_specified_remaining,
                    fee,
                )?;

            // Decrement the amount remaining to be swapped and amount received from the step
            current_state.amount_specified_remaining = current_state
                .amount_specified_remaining
                .overflowing_sub(I256::from_raw(step.amount_in.overflowing_add(step.fee_amount).0))
                .0;

            current_state.amount_calculated -= I256::from_raw(step.amount_out);

            // If the price moved all the way to the next price, recompute the liquidity change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net: i128 =
                        UniswapV3DBReader::ticks_liquidity_net(&db, pool_address, step.tick_next).unwrap_or_default();

                    // we are on a tick boundary, and the next tick is initialized, so we must charge a protocol fee
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Err(eyre!("LIQUIDITY_UNDERFLOW"));
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                // Increment the current tick
                current_state.tick = if zero_for_one { step.tick_next.wrapping_sub(1) } else { step.tick_next }
                // If the current_state sqrt price is not equal to the step sqrt price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = loom_defi_uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(current_state.sqrt_price_x_96)?;
            }
        }

        if current_state.amount_specified_remaining.is_zero() {
            let amount_out = (-current_state.amount_calculated).into_raw();
            tracing::trace!("AmountOut : {amount_out}");
            Ok(amount_out)
        } else {
            Err(eyre!("NOT_ENOUGH_LIQUIDITY"))
        }
    }

    pub fn simulate_swap_out_amount_provided<DB: DatabaseRef>(
        db: &DB,
        pool: &UniswapV3Pool,
        token_in: Address,
        amount_out: U256,
    ) -> eyre::Result<U256> {
        if amount_out.is_zero() {
            return Ok(U256::ZERO);
        }

        let zero_for_one = token_in == pool.get_tokens()[0];

        // Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one { MIN_SQRT_RATIO + U256_1 } else { MAX_SQRT_RATIO - U256_1 };

        let pool_address = pool.get_address();

        let slot0 = UniswapV3DBReader::slot0(&db, pool_address)?;
        let liquidity = UniswapV3DBReader::liquidity(db, pool_address)?;
        let tick_spacing = pool.tick_spacing();
        let fee = pool.fee;

        // Initialize a mutable state struct to hold the dynamic simulated state of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: slot0.sqrtPriceX96.to(),                //Active price on the pool
            amount_calculated: I256::ZERO,                           //Amount of token_out that has been calculated
            amount_specified_remaining: -I256::from_raw(amount_out), //Amount of token_in that has not been swapped
            tick: slot0.tick.as_i32(),                               //Current i24 tick of the pool
            liquidity,                                               //Current available liquidity in the tick range
        };

        while current_state.amount_specified_remaining != I256::ZERO && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96 {
            // Initialize a new step struct to hold the dynamic state of the pool at each step
            let mut step = StepComputations {
                // Set the sqrt_price_start_x_96 to the current sqrt_price_x_96
                sqrt_price_start_x_96: current_state.sqrt_price_x_96,
                ..Default::default()
            };

            let tick_provider = TickProviderEVMDB::new(&db, pool_address);

            // Get the next tick from the current tick
            (step.tick_next, step.initialized) = loom_defi_uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                &tick_provider,
                current_state.tick,
                tick_spacing as i32,
                zero_for_one,
            )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 = loom_defi_uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            // Target spot price
            let swap_target_sqrt_ratio = if zero_for_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            // Compute swap step and update the current state
            (current_state.sqrt_price_x_96, step.amount_in, step.amount_out, step.fee_amount) =
                loom_defi_uniswap_v3_math::swap_math::compute_swap_step(
                    current_state.sqrt_price_x_96,
                    swap_target_sqrt_ratio,
                    current_state.liquidity,
                    current_state.amount_specified_remaining,
                    fee,
                )?;

            // Decrement the amount remaining to be swapped and amount received from the step
            current_state.amount_specified_remaining =
                current_state.amount_specified_remaining.overflowing_add(I256::from_raw(step.amount_out)).0;

            current_state.amount_calculated =
                current_state.amount_calculated.overflowing_add(I256::from_raw(step.amount_in.overflowing_add(step.fee_amount).0)).0;

            // If the price moved all the way to the next price, recompute the liquidity change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net: i128 =
                        UniswapV3DBReader::ticks_liquidity_net(db, pool_address, step.tick_next).unwrap_or_default();

                    // we are on a tick boundary, and the next tick is initialized, so we must charge a protocol fee
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Err(eyre!("LIQUIDITY_UNDERFLOW"));
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                // Increment the current tick
                current_state.tick = if zero_for_one { step.tick_next.wrapping_sub(1) } else { step.tick_next }
                // If the current_state sqrt price is not equal to the step sqrt price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = loom_defi_uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(current_state.sqrt_price_x_96)?;
            }
        }

        if current_state.amount_specified_remaining.is_zero() {
            let amount_in = current_state.amount_calculated.into_raw();

            tracing::trace!("Amount In : {amount_in}");

            Ok(amount_in)
        } else {
            Err(eyre!("NOT_ENOUGH_LIQUIDITY"))
        }
    }
}

#[cfg(test)]
mod test {
    use alloy::primitives::U256;
    use loom_defi_uniswap_v3_math::full_math::mul_div_rounding_up;

    #[test]
    fn test_mul_rounding_up() {
        let amount = U256::from_limbs([1230267133767, 0, 0, 0]);
        let ret = mul_div_rounding_up(amount, U256::from(500), U256::from(1e6)).unwrap();
        assert_eq!(ret, U256::from(615133567u128));
    }
}
