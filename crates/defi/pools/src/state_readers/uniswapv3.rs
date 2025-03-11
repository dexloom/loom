use alloy::network::TransactionBuilder;
use alloy::primitives::aliases::U24;
use alloy::primitives::{Address, TxKind};
use alloy::rpc::types::TransactionRequest;
use alloy::sol_types::{SolCall, SolInterface};
use loom_defi_abi::uniswap3::IUniswapV3Pool;
use loom_defi_abi::uniswap3::IUniswapV3Pool::slot0Return;
use loom_evm_utils::{evm_call, evm_dyn_call, LoomExecuteEvm};
use revm::ExecuteEvm;

pub struct UniswapV3EvmStateReader {}

impl UniswapV3EvmStateReader {
    pub fn factory(evm: &mut dyn LoomExecuteEvm, pool: Address) -> eyre::Result<Address> {
        let input = IUniswapV3Pool::IUniswapV3PoolCalls::factory(IUniswapV3Pool::factoryCall {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV3Pool::factoryCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token0(evm: &mut dyn LoomExecuteEvm, pool: Address) -> eyre::Result<Address> {
        let input = IUniswapV3Pool::IUniswapV3PoolCalls::token0(IUniswapV3Pool::token0Call {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV3Pool::token0Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn token1(evm: &mut dyn LoomExecuteEvm, pool: Address) -> eyre::Result<Address> {
        let input = IUniswapV3Pool::IUniswapV3PoolCalls::token1(IUniswapV3Pool::token1Call {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV3Pool::token1Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn fee(evm: &mut dyn LoomExecuteEvm, pool: Address) -> eyre::Result<U24> {
        let input = IUniswapV3Pool::IUniswapV3PoolCalls::fee(IUniswapV3Pool::feeCall {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV3Pool::feeCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn tick_spacing(evm: &mut dyn LoomExecuteEvm, pool: Address) -> eyre::Result<u32> {
        let input = IUniswapV3Pool::IUniswapV3PoolCalls::tickSpacing(IUniswapV3Pool::tickSpacingCall {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV3Pool::tickSpacingCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0.try_into()?)
    }

    pub fn slot0(evm: &mut dyn LoomExecuteEvm, pool: Address) -> eyre::Result<slot0Return> {
        let input = IUniswapV3Pool::IUniswapV3PoolCalls::slot0(IUniswapV3Pool::slot0Call {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);
        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV3Pool::slot0Call::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return)
    }
    pub fn liquidity(evm: &mut dyn LoomExecuteEvm, pool: Address) -> eyre::Result<u128> {
        let input = IUniswapV3Pool::IUniswapV3PoolCalls::liquidity(IUniswapV3Pool::liquidityCall {}).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(pool)).with_input(input);

        let call_data_result = evm_dyn_call(evm, req)?.0;
        let call_return = IUniswapV3Pool::liquidityCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }
}
