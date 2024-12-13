use crate::protocols::helper::get_uniswap2pool_address;
use crate::protocols::protocol::Protocol;
use alloy::primitives::{Address, B256};
use loom_defi_address_book::FactoryAddress;

pub struct SushiswapProtocol {}

impl SushiswapProtocol {
    pub fn get_pool_address_for_tokens(token0: Address, token1: Address) -> Address {
        let init_code: B256 = "e18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303".parse().unwrap();
        get_uniswap2pool_address(token0, token1, FactoryAddress::SUSHISWAP_V2, init_code)
    }
}

impl Protocol for SushiswapProtocol {
    fn get_pool_address_vec_for_tokens(token0: Address, token1: Address) -> Vec<Address> {
        vec![SushiswapProtocol::get_pool_address_for_tokens(token0, token1)]
    }
}
