use crate::error::UniswapV3MathError;
use crate::full_math::mul_div;
use crate::sqrt_price_math::Q96;
use alloy::primitives::{U128, U256};
use eyre::eyre;

// returns (uint128 z)
pub fn add_delta(x: u128, y: i128) -> Result<u128, UniswapV3MathError> {
    if y < 0 {
        let z = x.overflowing_sub(-y as u128);

        if z.1 {
            Err(UniswapV3MathError::LiquiditySub)
        } else {
            Ok(z.0)
        }
    } else {
        let z = x.overflowing_add(y as u128);
        if z.0 < x {
            Err(UniswapV3MathError::LiquidityAdd)
        } else {
            Ok(z.0)
        }
    }
}

pub fn get_liquidity_for_amount0(sqrt_ratio_a_x_96: U256, sqrt_ratio_b_x_96: U256, amount0: U256) -> eyre::Result<u128> {
    let (sqrt_ratio_a_x_96, sqrt_ratio_b_x_96) =
        if sqrt_ratio_a_x_96 > sqrt_ratio_b_x_96 { (sqrt_ratio_b_x_96, sqrt_ratio_a_x_96) } else { (sqrt_ratio_a_x_96, sqrt_ratio_b_x_96) };

    //let mut denominator = Q96;
    let intermediate = mul_div(sqrt_ratio_a_x_96, sqrt_ratio_b_x_96, Q96)?;
    let ret = mul_div(amount0, intermediate, sqrt_ratio_b_x_96 - sqrt_ratio_a_x_96)?;
    if ret > U256::from(U128::MAX) {
        Err(eyre!("LIQUIDITY_OVERFLOWN"))
    } else {
        Ok(ret.to())
    }
}

pub fn get_liquidity_for_amount1(sqrt_ratio_a_x_96: U256, sqrt_ratio_b_x_96: U256, amount1: U256) -> eyre::Result<u128> {
    let (sqrt_ratio_a_x_96, sqrt_ratio_b_x_96) =
        if sqrt_ratio_a_x_96 > sqrt_ratio_b_x_96 { (sqrt_ratio_b_x_96, sqrt_ratio_a_x_96) } else { (sqrt_ratio_a_x_96, sqrt_ratio_b_x_96) };
    let ret = mul_div(amount1, Q96, sqrt_ratio_b_x_96 - sqrt_ratio_a_x_96)?;
    if ret > U256::from(U128::MAX) {
        Err(eyre!("LIQUIDITY_OVERFLOWN"))
    } else {
        Ok(ret.to())
    }
}

pub fn get_liquidity_for_amounts(
    sqrt_ratio_x_96: U256,
    sqrt_ratio_a_x_96: U256,
    sqrt_ratio_b_x_96: U256,
    amount0: U256,
    amount1: U256,
) -> eyre::Result<u128> {
    let (sqrt_ratio_a_x_96, sqrt_ratio_b_x_96) =
        if sqrt_ratio_a_x_96 > sqrt_ratio_b_x_96 { (sqrt_ratio_b_x_96, sqrt_ratio_a_x_96) } else { (sqrt_ratio_a_x_96, sqrt_ratio_b_x_96) };
    let liquidity = if sqrt_ratio_x_96 <= sqrt_ratio_a_x_96 {
        get_liquidity_for_amount0(sqrt_ratio_a_x_96, sqrt_ratio_b_x_96, amount0)?
    } else if sqrt_ratio_x_96 < sqrt_ratio_b_x_96 {
        let liquidity0 = get_liquidity_for_amount0(sqrt_ratio_x_96, sqrt_ratio_b_x_96, amount0)?;
        let liquidity1 = get_liquidity_for_amount1(sqrt_ratio_a_x_96, sqrt_ratio_x_96, amount1)?;
        if liquidity0 < liquidity1 {
            liquidity0
        } else {
            liquidity1
        }
    } else {
        get_liquidity_for_amount1(sqrt_ratio_a_x_96, sqrt_ratio_b_x_96, amount1)?
    };
    Ok(liquidity)
}

#[cfg(test)]
mod test {

    use crate::liquidity_math::add_delta;

    #[test]
    fn test_add_delta() {
        // 1 + 0
        let result = add_delta(1, 0);
        assert_eq!(result.unwrap(), 1);

        // 1 + -1
        let result = add_delta(1, -1);
        assert_eq!(result.unwrap(), 0);

        // 1 + 1
        let result = add_delta(1, 1);
        assert_eq!(result.unwrap(), 2);

        // 2**128-15 + 15 overflows
        let result = add_delta(340282366920938463463374607431768211441, 15);
        assert_eq!(result.err().unwrap().to_string(), "Liquidity Add");

        // 0 + -1 underflows
        let result = add_delta(0, -1);
        assert_eq!(result.err().unwrap().to_string(), "Liquidity Sub");

        // 3 + -4 underflows
        let result = add_delta(3, -4);
        assert_eq!(result.err().unwrap().to_string(), "Liquidity Sub");
    }
}
