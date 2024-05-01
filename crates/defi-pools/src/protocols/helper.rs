use std::ops::Add;

use alloy_primitives::{Address, B256, keccak256};
use alloy_provider::Provider;
use eyre::Result;
use revm::primitives::bitvec::view::BitViewSized;

use defi_abi::uniswap2::IUniswapV2Pair;
use defi_abi::uniswap3::IUniswapV3Pool;

fn sort_tokens(token0: Address, token1: Address) -> (Address, Address) {
    if token0 < token1 {
        (token0, token1)
    } else {
        (token1, token0)
    }
}

pub fn get_uniswap2pool_address(token0: Address, token1: Address, factory: Address, init_code: B256) -> Address {
    let mut buf: Vec<u8> = vec![0xFF];
    let mut addr_buf: Vec<u8> = Vec::new();
    let (token0, token1) = sort_tokens(token0, token1);


    addr_buf.extend_from_slice(token0.as_slice());
    addr_buf.extend_from_slice(token1.as_slice());

    let addr_hash = keccak256(addr_buf);
    buf.extend_from_slice(factory.as_ref());
    buf.extend_from_slice(addr_hash.as_ref());
    buf.extend_from_slice(init_code.as_ref());

    let hash = keccak256(buf);
    let ret: Address = Address::from_slice(hash.as_slice()[12..32].as_ref());
    ret
}


pub fn get_uniswap3pool_address(token0: Address, token1: Address, fee: u32, factory: Address, init_code: B256) -> Address {
    let mut buf: Vec<u8> = vec![0xFF];

    let mut addr_buf: Vec<u8> = Vec::new();
    let (token0, token1) = sort_tokens(token0, token1);

    let fee_buf: Vec<u8> = vec![
        ((fee >> 16) & 0xFF) as u8,
        ((fee >> 8) & 0xFF) as u8,
        ((fee) & 0xFF) as u8,
    ];

    addr_buf.extend([0u8; 12]);
    addr_buf.extend_from_slice(token0.as_slice());
    addr_buf.extend([0u8; 12]);
    addr_buf.extend_from_slice(token1.as_slice());
    addr_buf.extend([0u8; 29]);
    addr_buf.extend(fee_buf);


    let addr_hash = keccak256(addr_buf);
    buf.extend_from_slice(factory.as_ref());
    buf.extend_from_slice(addr_hash.as_ref());
    buf.extend_from_slice(init_code.as_ref());

    let hash = keccak256(buf);
    let ret: Address = Address::from_slice(hash.as_slice()[12..32].as_ref());
    ret
}


pub async fn fetch_uni2_factory<P: Provider>(client: P, address: Address) -> Result<Address> {
    let pool = IUniswapV2Pair::IUniswapV2PairInstance::new(address, client);
    let factory = pool.factory().call().await?;
    Ok(factory._0)
}

pub async fn fetch_uni3_factory<P: Provider>(client: P, address: Address) -> Result<Address> {
    let pool = IUniswapV3Pool::IUniswapV3PoolInstance::new(address, client);
    let factory = pool.factory().call().await?;
    Ok(factory._0)
}


#[cfg(test)]
mod test {
    use alloy_primitives::Address;
    use lazy_static::lazy_static;

    use super::*;

    lazy_static! {
            static ref weth_address : Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
            static ref  usdc_address : Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
            static ref  usdt_address : Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap();
            static ref  dai_address : Address = "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap();
            static ref  wbtc_address : Address = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse().unwrap();

    }

    #[test]
    fn test_get_uniswapv2_address() {
        let uni2_factory_address: Address = "0x5c69bee701ef814a2b6a3edd4b1652cb9cc5aa6f".parse().unwrap();
        let init_code: B256 = "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f".parse().unwrap();


        let pair_address = get_uniswap2pool_address(*weth_address, *usdc_address, uni2_factory_address, init_code);
        println!("{:?}", pair_address)
    }

    #[test]
    fn test_get_uniswapv3_address() {
        let uni3_factory_address: Address = "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse().unwrap();
        let init_code: B256 = "e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54".parse().unwrap();


        let pair_address = get_uniswap3pool_address(*weth_address, *usdc_address, 3000, uni3_factory_address, init_code);
        println!("{:?}", pair_address)
    }
}