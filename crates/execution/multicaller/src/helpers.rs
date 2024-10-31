use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::SolInterface;

use loom_defi_abi::balancer::IVault;
use loom_defi_abi::lido::{IStEth, IWStEth};
use loom_defi_abi::{IMultiCaller, IERC20, IWETH};
use loom_defi_address_book::TokenAddress;

pub struct EncoderHelper;

impl EncoderHelper {
    pub fn is_weth(address: Address) -> bool {
        address == TokenAddress::WETH
    }

    pub fn encode_weth_deposit() -> Bytes {
        IWETH::IWETHCalls::deposit(IWETH::depositCall {}).abi_encode().into()
    }

    pub fn encode_weth_withdraw(wad: U256) -> Bytes {
        IWETH::IWETHCalls::withdraw(IWETH::withdrawCall { wad }).abi_encode().into()
    }

    pub fn encode_erc20_transfer(to: Address, amount: U256) -> Bytes {
        IERC20::IERC20Calls::transfer(IERC20::transferCall { to, amount }).abi_encode().into()
    }

    pub fn encode_erc20_balance_of(account: Address) -> Bytes {
        IERC20::IERC20Calls::balanceOf(IERC20::balanceOfCall { account }).abi_encode().into()
    }

    pub fn encode_erc20_approve(spender: Address, amount: U256) -> Bytes {
        IERC20::IERC20Calls::approve(IERC20::approveCall { spender, amount }).abi_encode().into()
    }

    pub fn encode_multicaller_transfer_tips_weth(min_balance: U256, tips: U256, owner: Address) -> Bytes {
        IMultiCaller::IMultiCallerCalls::transferTipsMinBalanceWETH(IMultiCaller::transferTipsMinBalanceWETHCall {
            min_balance,
            tips,
            owner,
        })
        .abi_encode()
        .into()
    }
    pub fn encode_multicaller_transfer_tips(token: Address, min_balance: U256, tips: U256, owner: Address) -> Bytes {
        IMultiCaller::IMultiCallerCalls::transferTipsMinBalance(IMultiCaller::transferTipsMinBalanceCall {
            token,
            min_balance,
            tips,
            owner,
        })
        .abi_encode()
        .into()
    }

    pub fn encode_multicaller_uni2_get_in_amount(token_from: Address, token_to: Address, pool: Address, amount: U256, fee: U256) -> Bytes {
        let call = if fee.is_zero() || fee.to::<u32>() == 9970 {
            if token_from > token_to {
                IMultiCaller::IMultiCallerCalls::uni2GetInAmountFrom0(IMultiCaller::uni2GetInAmountFrom0Call { pool, amount })
            } else {
                IMultiCaller::IMultiCallerCalls::uni2GetInAmountFrom1(IMultiCaller::uni2GetInAmountFrom1Call { pool, amount })
            }
        } else if token_from > token_to {
            IMultiCaller::IMultiCallerCalls::uni2GetInAmountFrom0Comms(IMultiCaller::uni2GetInAmountFrom0CommsCall { pool, amount, fee })
        } else {
            IMultiCaller::IMultiCallerCalls::uni2GetInAmountFrom1Comms(IMultiCaller::uni2GetInAmountFrom1CommsCall { pool, amount, fee })
        };

        call.abi_encode().into()
    }

    pub fn encode_multicaller_uni2_get_out_amount(token_from: Address, token_to: Address, pool: Address, amount: U256, fee: U256) -> Bytes {
        let call = if fee.is_zero() || fee.to::<u32>() == 9970 {
            if token_from < token_to {
                IMultiCaller::IMultiCallerCalls::uni2GetOutAmountFrom0(IMultiCaller::uni2GetOutAmountFrom0Call { pool, amount })
            } else {
                IMultiCaller::IMultiCallerCalls::uni2GetOutAmountFrom1(IMultiCaller::uni2GetOutAmountFrom1Call { pool, amount })
            }
        } else if token_from < token_to {
            IMultiCaller::IMultiCallerCalls::uni2GetOutAmountFrom0Comms(IMultiCaller::uni2GetOutAmountFrom0CommsCall { pool, amount, fee })
        } else {
            IMultiCaller::IMultiCallerCalls::uni2GetOutAmountFrom1Comms(IMultiCaller::uni2GetOutAmountFrom1CommsCall { pool, amount, fee })
        };

        call.abi_encode().into()
    }

    pub fn encode_balancer_flashloan(token: Address, amount: U256, user_data: Bytes, recipient: Address) -> Bytes {
        let call = IVault::IVaultCalls::flashLoan(IVault::flashLoanCall {
            recipient,
            tokens: vec![token],
            amounts: vec![amount],
            userData: user_data,
        });

        Bytes::from(call.abi_encode())
    }

    pub fn encode_wsteth_wrap(st_eth_amount: U256) -> Bytes {
        let call = IWStEth::IWStEthCalls::wrap(IWStEth::wrapCall { stETHAmount: st_eth_amount });

        Bytes::from(call.abi_encode())
    }

    pub fn encode_wsteth_unwrap(wst_eth_amount: U256) -> Bytes {
        let call = IWStEth::IWStEthCalls::unwrap(IWStEth::unwrapCall { wstETHAmount: wst_eth_amount });

        Bytes::from(call.abi_encode())
    }

    pub fn encode_steth_submit(_amount: U256) -> Bytes {
        let call = IStEth::IStEthCalls::submit(IStEth::submitCall { _referral: Address::ZERO });

        Bytes::from(call.abi_encode())
    }
}
