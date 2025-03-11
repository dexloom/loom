use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, TxKind, U256};
use alloy::rpc::types::TransactionRequest;
use alloy::sol_types::{SolCall, SolInterface};
use eyre::{eyre, Result};
use loom_defi_abi::uniswap2::IUniswapV2Pair;
use loom_evm_utils::{evm_call, evm_dyn_call, LoomExecuteEvm};
use revm::DatabaseRef;
use tracing::error;

pub struct UniswapV2EVMStateReader {}

impl UniswapV2EVMStateReader {
    pub fn factory(evm: &mut dyn LoomExecuteEvm, pool: Address) -> Result<Address> {
        let input = IUniswapV2Pair::IUniswapV2PairCalls::factory(IUniswapV2Pair::factoryCall {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV2Pair::factoryCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token0(evm: &mut dyn LoomExecuteEvm, pool: Address) -> Result<Address> {
        let input = IUniswapV2Pair::IUniswapV2PairCalls::token0(IUniswapV2Pair::token0Call {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;

        let call_return = IUniswapV2Pair::token0Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token1(evm: &mut dyn LoomExecuteEvm, pool: Address) -> Result<Address> {
        let input = IUniswapV2Pair::IUniswapV2PairCalls::token1(IUniswapV2Pair::token1Call {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV2Pair::token1Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn get_reserves(evm: &mut dyn LoomExecuteEvm, pool: Address) -> Result<(U256, U256)> {
        let input = IUniswapV2Pair::IUniswapV2PairCalls::getReserves(IUniswapV2Pair::getReservesCall {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input).with_gas_limit(100_000);
        let call_data_result = match evm_dyn_call(evm, req) {
            Ok(call_data) => call_data.0,
            Err(error) => {
                error!(%error,"get_reserves");
                return Err(eyre!(error));
            }
        };

        let call_return = IUniswapV2Pair::getReservesCall::abi_decode_returns(&call_data_result, false)?;
        Ok((U256::from(call_return.reserve0), U256::from(call_return.reserve1)))
    }
}
