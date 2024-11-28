use alloy_primitives::Address;
use criterion::{criterion_group, criterion_main, Criterion};
use lazy_static::lazy_static;
use loom_defi_address_book::TokenAddressEth;
use loom_types_entities::{Market, MockPool, Pool, PoolWrapper, Token};
use std::collections::BTreeMap;
use std::sync::Arc;

lazy_static! {
    static ref WETH: Token = Token::new_with_data(TokenAddressEth::WETH, Some("WETH".to_string()), None, Some(18), true, false);
    static ref USDT: Token = Token::new_with_data(TokenAddressEth::USDT, Some("USDT".to_string()), None, Some(18), true, false);
}

fn create_pool(token0: Address, token1: Address) -> MockPool {
    MockPool::new(token0, token1, Address::random())
}

fn test_market_fill() -> eyre::Result<()> {
    let mut market = Market::default();
    //let mut market2 = Market::default();
    market.add_token(WETH.clone())?;
    market.add_token(USDT.clone())?;
    let weth_usdt_pool = create_pool(WETH.get_address(), USDT.get_address());
    market.add_pool(weth_usdt_pool)?;
    let weth_usdt_pool = create_pool(WETH.get_address(), USDT.get_address());
    market.add_pool(weth_usdt_pool)?;

    for _ in 0..1000 {
        let token_address = Address::random();
        let weth_pool = create_pool(WETH.get_address(), token_address);
        let usdt_pool = create_pool(USDT.get_address(), token_address);
        let mut btree = BTreeMap::default();
        for p in [&weth_pool, &usdt_pool] {
            btree.insert(PoolWrapper::new(Arc::new(p.clone())), p.get_swap_directions());
        }
        market.add_pool(weth_pool)?;
        market.add_pool(usdt_pool)?;
        let swap_paths = market.build_swap_path_vec(&btree)?;
        market.add_paths(swap_paths);
    }

    for _ in 0..1000 {
        let token_address = Address::random();
        let weth_pool = create_pool(WETH.get_address(), token_address);
        let usdt_pool = create_pool(USDT.get_address(), token_address);
        let mut btree = BTreeMap::default();
        for p in [&weth_pool, &usdt_pool] {
            btree.insert(PoolWrapper::new(Arc::new(p.clone())), p.get_swap_directions());
        }
        market.add_pool(weth_pool)?;
        market.add_pool(usdt_pool)?;
        let swap_paths = market.build_swap_path_vec(&btree)?;
        market.add_paths(swap_paths);
    }
    println!("{}", market);
    Ok(())
}

fn benchmark_test_group_hasher(c: &mut Criterion) {
    let mut group = c.benchmark_group("market");
    group.sample_size(10);

    group.bench_function("test_market_fill", |b| b.iter(test_market_fill));
    group.finish();
}

criterion_group!(benches, benchmark_test_group_hasher);
criterion_main!(benches);
/*
#[cfg(test)]
mod tests {
    use crate::test_market_fill;

    #[test]
    fn test() {
        test_market_fill().unwrap();
    }
}

fn main() {
    test_market_fill().unwrap();
}

 */
