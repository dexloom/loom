use alloy_primitives::aliases::{I24, U24};
use alloy_primitives::Address;
use alloy_sol_types::{SolCall, SolInterface};
use revm::primitives::Env;

use defi_abi::uniswap3::IUniswapV3Pool;
use defi_abi::uniswap3::IUniswapV3Pool::{slot0Return, ticksReturn};
use loom_revm_db::LoomInMemoryDB;
use loom_utils::evm::evm_call;

pub struct UniswapV3StateReader {}

impl UniswapV3StateReader {
    pub fn factory(db: &LoomInMemoryDB, env: Env, pool: Address) -> eyre::Result<Address> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::factory(IUniswapV3Pool::factoryCall {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::factoryCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token0(db: &LoomInMemoryDB, env: Env, pool: Address) -> eyre::Result<Address> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::token0(IUniswapV3Pool::token0Call {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::token0Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token1(db: &LoomInMemoryDB, env: Env, pool: Address) -> eyre::Result<Address> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::token1(IUniswapV3Pool::token1Call {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::token1Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn fee(db: &LoomInMemoryDB, env: Env, pool: Address) -> eyre::Result<U24> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::fee(IUniswapV3Pool::feeCall {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::feeCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn tick_spacing(db: &LoomInMemoryDB, env: Env, pool: Address) -> eyre::Result<u32> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::tickSpacing(IUniswapV3Pool::tickSpacingCall {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::tickSpacingCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0.try_into()?)
    }

    pub fn slot0(db: &LoomInMemoryDB, env: Env, pool: Address) -> eyre::Result<slot0Return> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::slot0(IUniswapV3Pool::slot0Call {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::slot0Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return)
    }
    pub fn liquidity(db: &LoomInMemoryDB, env: Env, pool: Address) -> eyre::Result<u128> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::liquidity(IUniswapV3Pool::liquidityCall {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::liquidityCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn ticks(db: &LoomInMemoryDB, env: Env, pool: Address, tick: I24) -> eyre::Result<ticksReturn> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::ticks(IUniswapV3Pool::ticksCall { tick }).abi_encode())?.0;
        let call_return = IUniswapV3Pool::ticksCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return)
    }
}
