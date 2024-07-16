use alloy_eips::eip1559::BaseFeeParams;

#[derive(Clone, Debug)]
pub struct ChainParameters {
    pub chain_id: u64,
    pub base_fee_params: BaseFeeParams,
}

impl ChainParameters {
    pub fn ethereum() -> ChainParameters {
        ChainParameters { chain_id: 1, base_fee_params: BaseFeeParams::ethereum() }
    }

    pub fn calc_next_block_base_fee(&self, gas_used: u128, gas_limit: u128, base_fee: u128) -> u128 {
        self.base_fee_params.next_block_base_fee(gas_used, gas_limit, base_fee)
    }
}
