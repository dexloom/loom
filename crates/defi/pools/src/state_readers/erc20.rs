use alloy::primitives::{Address, U256};
use alloy::sol_types::{SolCall, SolInterface};
use eyre::Result;
use loom_defi_abi::IERC20;
use loom_evm_utils::evm::evm_call;
use revm::primitives::Env;
use revm::DatabaseRef;

pub struct ERC20StateReader {}

#[allow(dead_code)]
impl ERC20StateReader {
    pub fn balance_of<DB: DatabaseRef>(db: &DB, env: Env, erc20_token: Address, account: Address) -> Result<U256> {
        let call_data_result =
            evm_call(db, env, erc20_token, IERC20::IERC20Calls::balanceOf(IERC20::balanceOfCall { account }).abi_encode())?.0;
        let call_return = IERC20::balanceOfCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn allowance<DB: DatabaseRef>(db: &DB, env: Env, erc20_token: Address, owner: Address, spender: Address) -> Result<U256> {
        let call_data_result =
            evm_call(db, env, erc20_token, IERC20::IERC20Calls::allowance(IERC20::allowanceCall { owner, spender }).abi_encode())?.0;
        let call_return = IERC20::allowanceCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }
}
