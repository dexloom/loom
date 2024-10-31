use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use alloy_sol_types::SolInterface;
use eyre::Result;
use loom_defi_abi::IERC20;
use loom_evm_db::LoomDBType;
use loom_evm_utils::evm::evm_call;
use revm::primitives::Env;

pub struct ERC20StateReader {}

#[allow(dead_code)]
impl ERC20StateReader {
    pub fn balance_of(db: &LoomDBType, env: Env, erc20_token: Address, account: Address) -> Result<U256> {
        let call_data_result =
            evm_call(db, env, erc20_token, IERC20::IERC20Calls::balanceOf(IERC20::balanceOfCall { account }).abi_encode())?.0;
        let call_return = IERC20::balanceOfCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }

    pub fn allowance(db: &LoomDBType, env: Env, erc20_token: Address, owner: Address, spender: Address) -> Result<U256> {
        let call_data_result =
            evm_call(db, env, erc20_token, IERC20::IERC20Calls::allowance(IERC20::allowanceCall { owner, spender }).abi_encode())?.0;
        let call_return = IERC20::allowanceCall::abi_decode_returns(&call_data_result, false)?;
        Ok(call_return._0)
    }
}
