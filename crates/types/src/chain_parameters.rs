use alloy_eips::eip1559::BaseFeeParams;
use alloy_rpc_types_eth::Header;

#[derive(Clone, Debug)]
pub struct ChainParameters {
    pub chain_id: u64,
    pub base_fee_params: BaseFeeParams,
}

impl ChainParameters {
    pub fn ethereum() -> ChainParameters {
        ChainParameters { chain_id: 1, base_fee_params: BaseFeeParams::ethereum() }
    }

    pub fn calc_next_block_base_fee(&self, gas_used: u64, gas_limit: u64, base_fee: u64) -> u64 {
        self.base_fee_params.next_block_base_fee(gas_used, gas_limit, base_fee)
    }

    pub fn calc_next_block_base_fee_from_header(&self, header: &Header) -> u64 {
        self.base_fee_params.next_block_base_fee(header.gas_used, header.gas_limit, header.base_fee_per_gas.unwrap_or_default())
    }
}

impl Default for ChainParameters {
    fn default() -> Self {
        Self::ethereum()
    }
}
impl From<u64> for ChainParameters {
    fn from(chain_id: u64) -> Self {
        match chain_id {
            1 => ChainParameters::ethereum(),
            _ => unimplemented!(),
        }
    }
}
