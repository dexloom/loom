use alloy_primitives::{address, BlockNumber};
use std::collections::BTreeMap;
use std::env;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use loom_defi_address_book::TokenAddressEth;
use loom_defi_pools::{UniswapV2Pool, UniswapV3Pool};
use loom_evm_db::LoomDBType;
use loom_node_debug_provider::AnvilDebugProviderFactory;
use loom_strategy_backrun::SwapCalculator;
use loom_types_entities::required_state::RequiredStateReader;
use loom_types_entities::{Market, PoolClass, PoolId, PoolWrapper, SwapLine, SwapPath, Token};
use revm::primitives::Env;

pub fn bench_swap_calculator(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_calculator");

    let pool_addresses = vec![
        (address!("322bba387c825180ebfb62bd8e6969ebe5b5e52d"), PoolClass::UniswapV2),
        (address!("f382839b955ab57cc1e041f2c987a909c9a48af1"), PoolClass::UniswapV2),
        (address!("49af5fb5de94c93ee83ad488fe8cab30b0ef35f2"), PoolClass::UniswapV3),
    ];

    let block_number = 20935488u64;
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();

    let mut state_db = LoomDBType::default();
    let (swap_path, state_db) = rt
        .block_on(async {
            let node_url = env::var("MAINNET_WS")?;
            let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(block_number)).await?;

            let mut market = Market::default();
            // Add basic token for start/end
            let weth_token = Token::new_with_data(TokenAddressEth::WETH, Some("WETH".to_string()), None, Some(18), true, false);
            market.add_token(weth_token)?;

            for (pool_address, pool_class) in pool_addresses.iter() {
                let pool: PoolWrapper;
                if pool_class == &PoolClass::UniswapV2 {
                    pool = UniswapV2Pool::fetch_pool_data(client.clone(), *pool_address).await?.into();
                } else if pool_class == &PoolClass::UniswapV3 {
                    pool = UniswapV3Pool::fetch_pool_data(client.clone(), *pool_address).await?.into();
                } else {
                    panic!("Unknown pool class");
                }

                let state_required = pool.get_state_required()?;
                let state_update = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, Some(block_number)).await?;
                state_db.apply_geth_update(state_update);
                let _ = market.add_pool(pool);
            }

            let mut directions = BTreeMap::new();
            let (pool_address, _) = pool_addresses.last().unwrap();
            let last_pool = market.get_pool(&PoolId::Address(*pool_address)).unwrap();
            directions.insert(last_pool.clone(), last_pool.get_swap_directions());
            let swap_path = market.build_swap_path_vec(&directions).unwrap().get(0).unwrap().clone();

            Ok::<(SwapPath, LoomDBType), eyre::Error>((swap_path, state_db.clone()))
        })
        .expect("Could not fetch state");

    rt.shutdown_background();

    let swap_line = SwapLine { path: swap_path, ..Default::default() };

    println!("SwapLine: {}", swap_line);
    group.bench_function("calculate", |b| {
        b.iter(|| {
            SwapCalculator::calculate(black_box(&mut swap_line.clone()), black_box(&state_db), black_box(Env::default()))
                .expect("Failed to calculate swap");
        })
    });

    group.finish();
}

criterion_group!(benches, bench_swap_calculator);
criterion_main!(benches);
