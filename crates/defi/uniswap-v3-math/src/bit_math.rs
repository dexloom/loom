use crate::error::UniswapV3MathError;
use alloy::primitives::U256;

pub fn most_significant_bit(x: U256) -> Result<u8, UniswapV3MathError> {
    if x.is_zero() {
        return Err(UniswapV3MathError::ZeroValue);
    }
    Ok(255 - x.leading_zeros() as u8)
}

pub fn least_significant_bit(x: U256) -> Result<u8, UniswapV3MathError> {
    if x.is_zero() {
        return Err(UniswapV3MathError::ZeroValue);
    }
    Ok(x.trailing_zeros() as u8)
}

#[cfg(test)]
mod test {
    use super::most_significant_bit;
    use crate::{bit_math::least_significant_bit, U256_1};
    use alloy::primitives::U256;
    use std::str::FromStr;

    #[test]
    fn test_most_significant_bit() {
        //0
        let result = most_significant_bit(U256::ZERO);
        assert_eq!(result.unwrap_err().to_string(), "Can not get most significant bit or least significant bit on zero value");

        //1
        let result = most_significant_bit(U256_1);
        assert_eq!(result.unwrap(), 0);

        //2
        let result = most_significant_bit(U256::from(2));
        assert_eq!(result.unwrap(), 1);

        //all powers of 2
        for i in 0..=255 {
            let result = most_significant_bit(U256::from(2).pow(U256::from(i)));
            assert_eq!(result.unwrap(), i as u8);
        }

        //uint256(-1)
        let result = most_significant_bit(
            //TODO:FIXME: might need to be from dec string
            U256::from_str("115792089237316195423570985008687907853269984665640564039457584007913129639935").unwrap(),
        );
        assert_eq!(result.unwrap(), 255);
    }

    #[test]
    fn test_least_significant_bit() {
        //0
        let result = least_significant_bit(U256::ZERO);
        assert_eq!(result.unwrap_err().to_string(), "Can not get most significant bit or least significant bit on zero value");

        //1
        let result = least_significant_bit(U256_1);
        assert_eq!(result.unwrap(), 0);

        //2
        let result = least_significant_bit(U256::from(2));
        assert_eq!(result.unwrap(), 1);

        //all powers of 2
        for i in 0..=255 {
            let result = least_significant_bit(U256::from(2).pow(U256::from(i)));
            assert_eq!(result.unwrap(), i as u8);
        }

        //uint256(-1)
        let result = least_significant_bit(
            //TODO:FIXME: might need to be from dec string
            U256::from_str("115792089237316195423570985008687907853269984665640564039457584007913129639935").unwrap(),
        );
        assert_eq!(result.unwrap(), 0);
    }
}
