use alloy::primitives::aliases::U24;
use alloy::primitives::{Address, U160, U256};
use alloy::sol_types::SolCall;
use eyre::{eyre, Result};
use loom_defi_abi::uniswap_periphery::IQuoterV2;
use loom_evm_utils::evm::evm_call;
use revm::primitives::Env;
use revm::DatabaseRef;

pub struct UniswapV3QuoterV2Encoder {}

impl UniswapV3QuoterV2Encoder {
    pub fn quote_exact_output_encode(token_in: Address, token_out: Address, fee: U24, price_limit: U160, amount_out: U256) -> Vec<u8> {
        let params = IQuoterV2::QuoteExactOutputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            amount: amount_out,
            fee,
            sqrtPriceLimitX96: price_limit,
        };
        let call = IQuoterV2::quoteExactOutputSingleCall { params };
        call.abi_encode()
    }

    pub fn quote_exact_input_encode(token_in: Address, token_out: Address, fee: U24, price_limit: U160, amount_in: U256) -> Vec<u8> {
        let params = IQuoterV2::QuoteExactInputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            amountIn: amount_in,
            fee,
            sqrtPriceLimitX96: price_limit,
        };
        let call = IQuoterV2::quoteExactInputSingleCall { params };
        call.abi_encode()
    }

    pub fn quote_exact_input_result_decode(data: &[u8]) -> Result<U256> {
        let ret = IQuoterV2::quoteExactInputSingleCall::abi_decode_returns(data, false);
        match ret {
            Ok(r) => Ok(r.amountOut),
            Err(_) => Err(eyre!("CANNOT_DECODE_EXACT_INPUT_RETURN")),
        }
    }
    pub fn quote_exact_output_result_decode(data: &[u8]) -> Result<U256> {
        let ret = IQuoterV2::quoteExactOutputSingleCall::abi_decode_returns(data, false);
        match ret {
            Ok(r) => Ok(r.amountIn),
            Err(_) => Err(eyre!("CANNOT_DECODE_EXACT_OUTPUT_RETURN")),
        }
    }
}

pub struct UniswapV3QuoterV2StateReader {}

impl UniswapV3QuoterV2StateReader {
    pub fn quote_exact_input<DB: DatabaseRef>(
        db: &DB,
        env: Env,
        quoter_address: Address,
        token_from: Address,
        token_to: Address,
        fee: U24,
        amount: U256,
    ) -> Result<(U256, u64)> {
        let call_data_vec = UniswapV3QuoterV2Encoder::quote_exact_input_encode(token_from, token_to, fee, U160::ZERO, amount);

        let (value, gas_used) = evm_call(db, env, quoter_address, call_data_vec)?;

        let ret = UniswapV3QuoterV2Encoder::quote_exact_input_result_decode(&value)?;
        Ok((ret, gas_used))
    }

    pub fn quote_exact_output<DB: DatabaseRef>(
        db: &DB,
        env: Env,
        quoter_address: Address,
        token_from: Address,
        token_to: Address,
        fee: U24,
        amount: U256,
    ) -> Result<(U256, u64)> {
        let call_data_vec = UniswapV3QuoterV2Encoder::quote_exact_output_encode(token_from, token_to, fee, U160::ZERO, amount);

        let (value, gas_used) = evm_call(db, env, quoter_address, call_data_vec)?;

        let ret = UniswapV3QuoterV2Encoder::quote_exact_output_result_decode(&value)?;
        Ok((ret, gas_used))
    }
}
