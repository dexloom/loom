use alloy::primitives::aliases::U24;
use alloy::primitives::Address;
use alloy::sol_types::{SolCall, SolInterface};
use revm::primitives::Env;
use revm::DatabaseRef;

use loom_defi_abi::uniswap3::IUniswapV3Pool;
use loom_defi_abi::uniswap3::IUniswapV3Pool::slot0Return;
use loom_evm_utils::evm::evm_call;

pub struct UniswapV3StateReader {}

impl UniswapV3StateReader {
    pub fn factory<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> eyre::Result<Address> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::factory(IUniswapV3Pool::factoryCall {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::factoryCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token0<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> eyre::Result<Address> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::token0(IUniswapV3Pool::token0Call {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::token0Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token1<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> eyre::Result<Address> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::token1(IUniswapV3Pool::token1Call {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::token1Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn fee<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> eyre::Result<U24> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::fee(IUniswapV3Pool::feeCall {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::feeCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn tick_spacing<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> eyre::Result<u32> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::tickSpacing(IUniswapV3Pool::tickSpacingCall {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::tickSpacingCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0.try_into()?)
    }

    pub fn slot0<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> eyre::Result<slot0Return> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::slot0(IUniswapV3Pool::slot0Call {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::slot0Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return)
    }
    pub fn liquidity<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> eyre::Result<u128> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV3Pool::IUniswapV3PoolCalls::liquidity(IUniswapV3Pool::liquidityCall {}).abi_encode())?.0;
        let call_return = IUniswapV3Pool::liquidityCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }
}
