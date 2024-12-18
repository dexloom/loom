use alloy::primitives::{b256, Address, Bytes, B256};
use alloy::sol_types::SolCall;
use loom_defi_abi::uniswap3::IUniswapV3Pool;
use loom_defi_address_book::FactoryAddress;

use crate::protocols::helper::get_uniswap3pool_address;
use crate::protocols::match_abi;
use crate::protocols::protocol::Protocol;

pub struct UniswapV3Protocol {}

impl UniswapV3Protocol {
    pub fn get_pool_address_for_tokens(token0: Address, token1: Address, fee: u32) -> Address {
        let init_code: B256 = b256!("e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54");

        get_uniswap3pool_address(token0, token1, fee, FactoryAddress::UNISWAP_V3, init_code)
    }

    pub fn is_code(code: &Bytes) -> bool {
        match_abi(code, vec![IUniswapV3Pool::swapCall::SELECTOR, IUniswapV3Pool::mintCall::SELECTOR, IUniswapV3Pool::collectCall::SELECTOR])
    }
}

impl Protocol for UniswapV3Protocol {
    fn get_pool_address_vec_for_tokens(token0: Address, token1: Address) -> Vec<Address> {
        let init_code: B256 = "e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54".parse().unwrap();

        let pair_address0 = get_uniswap3pool_address(token0, token1, 100, FactoryAddress::UNISWAP_V3, init_code);
        let pair_address1 = get_uniswap3pool_address(token0, token1, 500, FactoryAddress::UNISWAP_V3, init_code);
        let pair_address2 = get_uniswap3pool_address(token0, token1, 3000, FactoryAddress::UNISWAP_V3, init_code);
        let pair_address3 = get_uniswap3pool_address(token0, token1, 10000, FactoryAddress::UNISWAP_V3, init_code);

        vec![pair_address0, pair_address1, pair_address2, pair_address3]
    }
}
