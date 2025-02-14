use alloy::primitives::{keccak256, Address, Bytes, B256};
use alloy::providers::{Network, Provider};
use eyre::Result;

use loom_defi_abi::uniswap2::IUniswapV2Pair;
use loom_defi_abi::uniswap3::IUniswapV3Pool;

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

    let fee_buf: Vec<u8> = vec![((fee >> 16) & 0xFF) as u8, ((fee >> 8) & 0xFF) as u8, ((fee) & 0xFF) as u8];

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

pub async fn fetch_uni2_factory<N: Network, P: Provider<N>>(client: P, address: Address) -> Result<Address> {
    let pool = IUniswapV2Pair::IUniswapV2PairInstance::new(address, client);
    let factory = pool.factory().call().await?;
    Ok(factory._0)
}

pub async fn fetch_uni3_factory<N: Network, P: Provider<N>>(client: P, address: Address) -> Result<Address> {
    let pool = IUniswapV3Pool::IUniswapV3PoolInstance::new(address, client);
    let factory = pool.factory().call().await?;
    Ok(factory._0)
}

pub fn match_abi(code: &Bytes, selectors: Vec<[u8; 4]>) -> bool {
    //println!("Code len {}", code.len());
    for selector in selectors.iter() {
        if !code.as_ref().windows(4).any(|sig| sig == selector) {
            //println!("{:?} not found", selector);
            return false;
        } else {
            //println!("{} found", fn_name);
        }
    }
    true
}

#[cfg(test)]
mod test {
    use super::*;
    use loom_defi_address_book::{FactoryAddress, TokenAddressEth};

    #[test]
    fn test_get_uniswapv2_address() {
        let init_code: B256 = "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f".parse().unwrap();

        let pair_address = get_uniswap2pool_address(TokenAddressEth::WETH, TokenAddressEth::USDC, FactoryAddress::UNISWAP_V2, init_code);
        println!("{:?}", pair_address)
    }

    #[test]
    fn test_get_uniswapv3_address() {
        let init_code: B256 = "e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54".parse().unwrap();

        let pair_address =
            get_uniswap3pool_address(TokenAddressEth::WETH, TokenAddressEth::USDC, 3000, FactoryAddress::UNISWAP_V3, init_code);
        println!("{:?}", pair_address)
    }
}
