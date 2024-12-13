use alloy::primitives::{Address, U256};
use alloy::sol_types::{SolCall, SolInterface};
use eyre::Result;
use revm::primitives::Env;
use revm::DatabaseRef;

use loom_defi_abi::uniswap2::IUniswapV2Pair;
use loom_evm_utils::evm::evm_call;

pub struct UniswapV2StateReader {}

impl UniswapV2StateReader {
    pub fn factory<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> Result<Address> {
        //info!(" ----- {} {} {}", db.storage(pool, U256::try_from(8).unwrap()).unwrap(), db.storage(pool, U256::try_from(9).unwrap()).unwrap(),  db.storage(pool, U256::try_from(10).unwrap()).unwrap());
        let call_data_result =
            evm_call(db, env, pool, IUniswapV2Pair::IUniswapV2PairCalls::factory(IUniswapV2Pair::factoryCall {}).abi_encode())?.0;
        let call_return = IUniswapV2Pair::factoryCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token0<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> Result<Address> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV2Pair::IUniswapV2PairCalls::token0(IUniswapV2Pair::token0Call {}).abi_encode())?.0;
        let call_return = IUniswapV2Pair::token0Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token1<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> Result<Address> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV2Pair::IUniswapV2PairCalls::token1(IUniswapV2Pair::token1Call {}).abi_encode())?.0;
        let call_return = IUniswapV2Pair::token1Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }
    /*
       pub fn is_code(code: &Bytecode) -> bool {
           match_abi(code, vec![IUniswapV2Pair::swapCall::SELECTOR, IUniswapV2Pair::mintCall::SELECTOR, IUniswapV2Pair::syncCall::SELECTOR, IUniswapV2Pair::token0Call::SELECTOR, IUniswapV2Pair::factoryCall::SELECTOR])
       }


    */
    pub fn get_reserves<DB: DatabaseRef>(db: &DB, env: Env, pool: Address) -> Result<(U256, U256)> {
        let call_data_result =
            evm_call(db, env, pool, IUniswapV2Pair::IUniswapV2PairCalls::getReserves(IUniswapV2Pair::getReservesCall {}).abi_encode())?.0;
        let call_return = IUniswapV2Pair::getReservesCall::abi_decode_returns(&call_data_result, false)?;
        Ok((U256::from(call_return.reserve0), U256::from(call_return.reserve1)))
    }
}
