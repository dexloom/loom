use alloy_primitives::{Address, B256, Bytes};
use alloy_sol_types::SolCall;

use defi_abi::uniswap2::IUniswapV2Pair;

use crate::protocols::helper::get_uniswap2pool_address;
use crate::protocols::match_abi;
use crate::protocols::protocol::Protocol;

pub struct UniswapV2Protocol {}

impl UniswapV2Protocol {
    pub fn is_code(code: &Bytes) -> bool {
        match_abi(code, vec![IUniswapV2Pair::swapCall::SELECTOR, IUniswapV2Pair::mintCall::SELECTOR, IUniswapV2Pair::syncCall::SELECTOR, IUniswapV2Pair::token0Call::SELECTOR, IUniswapV2Pair::factoryCall::SELECTOR])
    }

    pub fn get_pool_address_for_tokens(token0: Address, token1: Address) -> Address {
        let uni2_factory_address: Address = "0x5c69bee701ef814a2b6a3edd4b1652cb9cc5aa6f".parse().unwrap();
        let init_code: B256 = "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f".parse().unwrap();
        let pair_address = get_uniswap2pool_address(token0, token1, uni2_factory_address, init_code);
        pair_address
    }
}

impl Protocol for UniswapV2Protocol {
    fn get_pool_address_vec_for_tokens(token0: Address, token1: Address) -> Vec<Address> {
        vec![UniswapV2Protocol::get_pool_address_for_tokens(token0, token1)]
    }
}

