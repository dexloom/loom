use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, TxKind, U256};
use alloy::rpc::types::TransactionRequest;
use alloy::sol_types::{SolCall, SolInterface};
use eyre::Result;
use loom_defi_abi::IERC20;
use loom_evm_utils::{evm_call, LoomExecuteEvm};

pub struct ERC20StateReader {}

#[allow(dead_code)]
impl ERC20StateReader {
    pub fn balance_of<EVM: LoomExecuteEvm>(evm: &mut EVM, erc20_token: Address, account: Address) -> Result<U256> {
        let input = IERC20::IERC20Calls::balanceOf(IERC20::balanceOfCall { account }).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(erc20_token)).with_input(input);
        let call_data_result = evm_call(evm, req)?.0;
        let call_return = IERC20::balanceOfCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn allowance<EVM: LoomExecuteEvm>(evm: &mut EVM, erc20_token: Address, owner: Address, spender: Address) -> Result<U256> {
        let input = IERC20::IERC20Calls::allowance(IERC20::allowanceCall { owner, spender }).abi_encode();
        let req = TransactionRequest::default().with_kind(TxKind::Call(erc20_token)).with_input(input);
        let call_data_result = evm_call(evm, req)?.0;
        let call_return = IERC20::allowanceCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }
}
