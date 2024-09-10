use alloy::primitives::U256;

use crate::U256_1;

pub fn div_rounding_up(a: U256, b: U256) -> U256 {
    let quotient = a.wrapping_div(b);
    let remainder = a.wrapping_rem(b);
    if remainder.is_zero() {
        quotient
    } else {
        quotient + U256_1
    }
}
