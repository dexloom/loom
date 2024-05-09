use alloy_primitives::U256;

#[derive(Clone, Debug, Default)]
pub struct GasStation {
    pub next_block_base_fee: u128,
}

impl GasStation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_next_base_fee(&self) -> u128 {
        self.next_block_base_fee
    }

    pub fn calc_gas_cost(gas: u128, gas_price: u128) -> U256 {
        U256::from(gas) * U256::from(gas_price)
    }

    pub fn get_gas_cost(&self, gas: u128) -> U256 {
        Self::calc_gas_cost(gas, self.next_block_base_fee)
    }
}