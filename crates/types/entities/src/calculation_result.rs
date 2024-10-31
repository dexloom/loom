use alloy_primitives::U256;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct CalculationResult {
    pub amount_in: U256,
    pub amount_out: U256,
}

impl CalculationResult {
    pub fn new(amount_in: U256, amount_out: U256) -> Self {
        Self { amount_in, amount_out }
    }
}

impl Display for CalculationResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "(amount_in={}, amount_out={})", self.amount_in, self.amount_out)
    }
}
