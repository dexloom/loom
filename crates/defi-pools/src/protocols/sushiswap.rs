use alloy_primitives::{Address, B256};

use crate::protocols::helper::get_uniswap2pool_address;
use crate::protocols::protocol::Protocol;

pub struct SushiswapProtocol {}

impl SushiswapProtocol {
    pub fn get_pool_address_for_tokens(token0: Address, token1: Address) -> Address {
        let uni2_factory_address: Address = "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse().unwrap();
        let init_code: B256 = "e18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303".parse().unwrap();
        get_uniswap2pool_address(token0, token1, uni2_factory_address, init_code)
    }
}

impl Protocol for SushiswapProtocol {
    fn get_pool_address_vec_for_tokens(token0: Address, token1: Address) -> Vec<Address> {
        vec![SushiswapProtocol::get_pool_address_for_tokens(token0, token1)]
    }
}
