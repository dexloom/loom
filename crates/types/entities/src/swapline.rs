use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use alloy_primitives::{Address, I256, U256};
use eyre::{eyre, ErrReport, Result};
use revm::primitives::Env;
use revm::DatabaseRef;
use tracing::debug;

use loom_types_blockchain::SwapError;

use crate::swappath::SwapPath;
use crate::{CalculationResult, PoolWrapper, SwapStep, Token};

#[derive(Clone, Copy, Debug, Default)]
pub enum SwapAmountType {
    #[default]
    NotSet,
    Set(U256),
    Stack0,
    RelativeStack(u32),
    Balance(Address),
}

impl SwapAmountType {
    pub fn unwrap(&self) -> U256 {
        match &self {
            Self::Set(x) => *x,
            _ => panic!("called `InAmountType::unwrap()` on a unknown value"),
        }
    }
    pub fn unwrap_or_zero(&self) -> U256 {
        match &self {
            Self::Set(x) => *x,
            _ => U256::ZERO,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SwapLine {
    pub path: SwapPath,
    /// Input token amount of the swap
    pub amount_in: SwapAmountType,
    /// Output token amount of the swap
    pub amount_out: SwapAmountType,
    /// The in and out amounts for each swap step
    pub calculation_results: Vec<CalculationResult>,
    /// Output token of the swap
    pub swap_to: Option<Address>,
    /// Gas used for the swap
    pub gas_used: Option<u64>,
}

impl fmt::Display for SwapLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token_in = self.tokens().first();
        let token_out = self.tokens().last();

        let profit: String = if token_in == token_out {
            match token_in {
                Some(t) => format!("profit={}", t.to_float_sign(self.profit().unwrap_or(I256::ZERO))),
                _ => format!("profit={}", self.profit().unwrap_or(I256::ZERO)),
            }
        } else {
            "-".to_string()
        };

        let tokens = self.tokens().iter().map(|token| token.get_symbol()).collect::<Vec<String>>().join(", ");
        let pools = self
            .pools()
            .iter()
            .map(|pool| format!("{}@{:#20x}", pool.get_protocol(), pool.get_address()))
            .collect::<Vec<String>>()
            .join(", ");
        let amount_in = match self.amount_in {
            SwapAmountType::Set(x) => match token_in {
                Some(t) => format!("{:?}", t.to_float(x)),
                _ => format!("{}", x),
            },
            _ => {
                format!("{:?}", self.amount_in)
            }
        };
        let amount_out = match self.amount_out {
            SwapAmountType::Set(x) => match token_out {
                Some(t) => format!("{:?}", t.to_float(x)),
                _ => format!("{}", x),
            },
            _ => {
                format!("{:?}", self.amount_out)
            }
        };

        let calculation_results =
            self.calculation_results.iter().map(|calculation_result| format!("{}", calculation_result)).collect::<Vec<String>>().join(", ");

        write!(
            f,
            "SwapLine [{}, tokens=[{}], pools=[{}], amount_in={}, amount_out={}, calculation_results=[{}], gas_used={:?}]",
            profit, tokens, pools, amount_in, amount_out, calculation_results, self.gas_used
        )
    }
}

impl Hash for SwapLine {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tokens().hash(state);
        self.pools().hash(state);
    }
}

impl PartialEq for SwapLine {
    fn eq(&self, other: &Self) -> bool {
        self.tokens() == other.tokens() && self.pools() == other.pools()
    }
}

impl From<SwapPath> for SwapLine {
    fn from(value: SwapPath) -> Self {
        Self { path: value, ..Default::default() }
    }
}

impl SwapLine {
    pub fn to_error(&self, msg: String) -> SwapError {
        SwapError {
            msg,
            pool: self.get_first_pool().map_or(Address::ZERO, |x| x.get_address()),
            token_from: self.get_first_token().map_or(Address::ZERO, |x| x.get_address()),
            token_to: self.get_last_token().map_or(Address::ZERO, |x| x.get_address()),
            is_in_amount: true,
            amount: self.amount_in.unwrap_or_zero(),
        }
    }

    pub fn new() -> Self {
        SwapLine::default()
    }

    /// Check if the path contains a specific pool
    pub fn contains_pool(&self, pool: &PoolWrapper) -> bool {
        self.path.contains_pool(pool)
    }

    /// Get all involved tokens in the swap line
    pub fn tokens(&self) -> &Vec<Arc<Token>> {
        &self.path.tokens
    }

    /// Get all used pools in the swap line
    pub fn pools(&self) -> &Vec<PoolWrapper> {
        &self.path.pools
    }

    /// Get the first token in the swap line
    pub fn get_first_token(&self) -> Option<&Arc<Token>> {
        self.tokens().first()
    }

    /// Get the last token in the swap line
    pub fn get_last_token(&self) -> Option<&Arc<Token>> {
        self.tokens().last()
    }

    /// Get the first pool in the swap line
    pub fn get_first_pool(&self) -> Option<&PoolWrapper> {
        self.pools().first()
    }

    /// Get the last pool in the swap line
    pub fn get_last_pool(&self) -> Option<&PoolWrapper> {
        self.pools().last()
    }

    /// Convert the swap line to two swap steps for flash swapping
    pub fn to_swap_steps(&self, multicaller: Address) -> Option<(SwapStep, SwapStep)> {
        let mut sp0: Option<SwapLine> = None;
        let mut sp1: Option<SwapLine> = None;

        for i in 1..self.path.pool_count() {
            let (flash_path, inside_path) = self.split(i).unwrap();
            if flash_path.can_flash_swap() || inside_path.can_flash_swap() {
                sp0 = Some(flash_path);
                sp1 = Some(inside_path);
                break;
            }
        }

        if sp0.is_none() || sp1.is_none() {
            let (flash_path, inside_path) = self.split(1).unwrap();
            sp0 = Some(flash_path);
            sp1 = Some(inside_path);
        }

        let mut step_0 = SwapStep::new(multicaller);
        step_0.add(sp0.unwrap());

        let mut step_1 = SwapStep::new(multicaller);
        let mut sp1 = sp1.unwrap();
        sp1.amount_in = SwapAmountType::Balance(multicaller);
        step_1.add(sp1);

        Some((step_0, step_1))
    }

    /// Split the swap line into two swap lines at a specific pool index
    pub fn split(&self, pool_index: usize) -> Result<(SwapLine, SwapLine)> {
        let first = SwapLine {
            path: SwapPath::new(self.tokens()[0..pool_index + 1].to_vec(), self.pools()[0..pool_index].to_vec()),
            amount_in: self.amount_in,
            amount_out: SwapAmountType::NotSet,
            calculation_results: vec![],
            swap_to: None,
            gas_used: None,
        };
        let second = SwapLine {
            path: SwapPath::new(self.tokens()[pool_index..].to_vec(), self.pools()[pool_index..].to_vec()),
            amount_in: SwapAmountType::NotSet,
            amount_out: self.amount_out,
            calculation_results: vec![],
            swap_to: None,
            gas_used: None,
        };
        Ok((first, second))
    }

    /// Check if all pools in the swap line can be flash swapped
    pub fn can_flash_swap(&self) -> bool {
        for pool in self.pools().iter() {
            if !pool.can_flash_swap() {
                return false;
            }
        }
        true
    }

    /// Calculate the absolute profit of the swap line
    pub fn abs_profit(&self) -> U256 {
        let Some(token_in) = self.tokens().first() else {
            return U256::ZERO;
        };
        let Some(token_out) = self.tokens().last() else {
            return U256::ZERO;
        };
        if token_in != token_out {
            return U256::ZERO;
        }
        let SwapAmountType::Set(amount_in) = self.amount_in else {
            return U256::ZERO;
        };
        let SwapAmountType::Set(amount_out) = self.amount_out else {
            return U256::ZERO;
        };
        if amount_out > amount_in {
            return amount_out - amount_in;
        }

        U256::ZERO
    }

    /// Calculate the absolute profit of the swap line in ETH
    pub fn abs_profit_eth(&self) -> U256 {
        let profit = self.abs_profit();
        let Some(first_token) = self.get_first_token() else {
            return U256::ZERO;
        };
        first_token.calc_eth_value(profit).unwrap_or(U256::ZERO)
    }

    pub fn profit(&self) -> Result<I256> {
        if self.tokens().len() < 3 {
            return Err(eyre!("NOT_ARB_PATH"));
        }
        if let Some(token_in) = self.tokens().first() {
            if let Some(token_out) = self.tokens().last() {
                return if token_in == token_out {
                    if let SwapAmountType::Set(amount_in) = self.amount_in {
                        if let SwapAmountType::Set(amount_out) = self.amount_out {
                            return Ok(I256::from_raw(amount_out) - I256::from_raw(amount_in));
                        }
                    }
                    Err(eyre!("AMOUNTS_NOT_SET"))
                } else {
                    Err(eyre!("TOKENS_DONT_MATCH"))
                };
            }
        }
        Err(eyre!("CANNOT_CALCULATE"))
    }

    /// Calculate the out amount for the swap line for a given in amount
    pub fn calculate_with_in_amount<DB: DatabaseRef<Error = ErrReport>>(
        &self,
        state: &DB,
        env: Env,
        in_amount: U256,
    ) -> Result<(U256, u64, Vec<CalculationResult>), SwapError> {
        let mut current_in_amount = in_amount;
        let mut final_out_amount = U256::ZERO;
        let mut gas_used = 0;
        let mut calculation_results = vec![];

        for (i, pool) in self.pools().iter().enumerate() {
            let token_from = &self.tokens()[i];
            let token_to = &self.tokens()[i + 1];
            match pool.calculate_out_amount(state, env.clone(), &token_from.get_address(), &token_to.get_address(), current_in_amount) {
                Ok((out_amount_result, gas_result)) => {
                    if out_amount_result.is_zero() {
                        return Err(SwapError {
                            msg: "ZERO_AMOUNT".to_string(),
                            pool: pool.get_address(),
                            token_from: token_from.get_address(),
                            token_to: token_to.get_address(),
                            is_in_amount: true,
                            amount: current_in_amount,
                        });
                    }
                    calculation_results.push(CalculationResult::new(current_in_amount, out_amount_result));
                    current_in_amount = out_amount_result;
                    final_out_amount = out_amount_result;
                    gas_used += gas_result
                }
                Err(e) => {
                    //error!("calculate_with_in_amount calculate_out_amount error {} amount {} : {}", self, in_amount, e);
                    return Err(SwapError {
                        msg: e.to_string(),
                        pool: pool.get_address(),
                        token_from: token_from.get_address(),
                        token_to: token_to.get_address(),
                        is_in_amount: true,
                        amount: current_in_amount,
                    });
                }
            }
        }
        Ok((final_out_amount, gas_used, calculation_results))
    }

    /// Calculate the in amount for the swap line for a given out amount
    pub fn calculate_with_out_amount(
        &self,
        state: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        out_amount: U256,
    ) -> Result<(U256, u64, Vec<CalculationResult>), SwapError> {
        let mut current_out_amount = out_amount;
        let mut final_in_amount = U256::ZERO;
        let mut gas_used = 0;
        let mut calculation_results = vec![];

        // TODO: Check if possible without clone?
        let mut pool_reverse = self.pools().clone();
        pool_reverse.reverse();
        let mut tokens_reverse = self.tokens().clone();
        tokens_reverse.reverse();

        for (i, pool) in pool_reverse.iter().enumerate() {
            let token_from = &tokens_reverse[i + 1];
            let token_to = &tokens_reverse[i];
            match pool.calculate_in_amount(state, env.clone(), &token_from.get_address(), &token_to.get_address(), current_out_amount) {
                Ok((in_amount_result, gas_result)) => {
                    if in_amount_result == U256::MAX || in_amount_result == U256::ZERO {
                        return Err(SwapError {
                            msg: "ZERO_AMOUNT".to_string(),
                            pool: pool.get_address(),
                            token_from: token_from.get_address(),
                            token_to: token_to.get_address(),
                            is_in_amount: false,
                            amount: current_out_amount,
                        });
                    }
                    calculation_results.push(CalculationResult::new(current_out_amount, in_amount_result));
                    current_out_amount = in_amount_result;
                    final_in_amount = in_amount_result;
                    gas_used += gas_result;
                }
                Err(e) => {
                    //error!("calculate_with_out_amount calculate_in_amount error {} amount {} : {}", self, in_amount, e);

                    return Err(SwapError {
                        msg: e.to_string(),
                        pool: pool.get_address(),
                        token_from: token_from.get_address(),
                        token_to: token_to.get_address(),
                        is_in_amount: false,
                        amount: current_out_amount,
                    });
                }
            }
        }
        Ok((final_in_amount, gas_used, calculation_results))
    }

    /// Optimize the swap line for a given in amount
    pub fn optimize_with_in_amount<DB: DatabaseRef<Error = ErrReport>>(
        &mut self,
        state: &DB,
        env: Env,
        in_amount: U256,
    ) -> Result<&mut Self, SwapError> {
        let mut current_in_amount = in_amount;
        let mut best_profit: Option<I256> = None;
        let mut current_step = U256::from(10000);
        let mut inc_direction = true;
        let mut first_step_change = false;
        let mut next_amount = current_in_amount;
        let mut prev_in_amount = U256::ZERO;
        let mut counter = 0;
        let denominator = U256::from(1000);

        loop {
            counter += 1;
            //let next_amount  = current_in_amount + (current_in_amount * current_step / 10000);

            if counter > 30 {
                debug!("optimize_swap_path_in_amount iterations exceeded : {self} {current_in_amount} {current_step}");
                return Ok(self);
            }

            let (current_out_amount, current_gas_used, calculation_results) =
                match self.calculate_with_in_amount(state, env.clone(), next_amount) {
                    Ok(ret) => ret,
                    Err(e) => {
                        if counter == 1 {
                            // break if first swap already fails
                            return Err(e);
                        }
                        (U256::ZERO, 0, vec![])
                    }
                };

            let current_profit = I256::from_raw(current_out_amount) - I256::from_raw(next_amount);

            if best_profit.is_none() {
                best_profit = Some(current_profit);
                self.amount_in = SwapAmountType::Set(next_amount);
                self.amount_out = SwapAmountType::Set(current_out_amount);
                self.gas_used = Some(current_gas_used);
                self.calculation_results = calculation_results;
                current_in_amount = next_amount;
                if current_out_amount.is_zero() || current_profit.is_negative() {
                    return Ok(self);
                }
            } else if best_profit.unwrap() > current_profit || current_out_amount.is_zero()
            /*|| next_profit < current_profit*/
            {
                if first_step_change && inc_direction && current_step < denominator {
                    inc_direction = false;
                    //TODO : Check why not used
                    next_amount = prev_in_amount;
                    current_in_amount = prev_in_amount;
                    first_step_change = true;
                    //debug!("inc direction changed {} {} {}", next_amount, current_profit, bestprofit.unwrap());
                } else if first_step_change && !inc_direction {
                    //TODO : Check why is self aligned
                    inc_direction = true;
                    current_step /= U256::from(10);
                    best_profit = Some(current_profit);
                    first_step_change = true;
                    //debug!("dec direction changed  {} {} {}", next_amount, current_profit, bestprofit.unwrap());

                    if current_step == U256::from(1) {
                        break;
                    }
                } else {
                    current_step /= U256::from(10);
                    first_step_change = true;
                    if current_step == U256::from(1) {
                        break;
                    }
                }
            } else {
                best_profit = Some(current_profit);
                self.amount_in = SwapAmountType::Set(next_amount);
                self.amount_out = SwapAmountType::Set(current_out_amount);
                self.gas_used = Some(current_gas_used);
                self.calculation_results = calculation_results;
                current_in_amount = next_amount;
                first_step_change = false;
            }

            prev_in_amount = current_in_amount;
            if inc_direction {
                next_amount = current_in_amount + (current_in_amount * current_step / denominator);
            } else {
                next_amount = current_in_amount - (current_in_amount * current_step / denominator);
            }
            //trace!("opt step : {} direction {} first_step {} step : {} current_in_amount : {} next_amount: {} profit : {} {}", counter, inc_direction, first_step_change,  current_step, current_in_amount , next_amount, current_profit, bestprofit.unwrap());
        }

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_pool::MockPool;
    use alloy_primitives::utils::parse_units;
    use loom_defi_address_book::{TokenAddress, UniswapV2PoolAddress, UniswapV3PoolAddress};
    use std::sync::Arc;

    fn default_swap_line() -> (MockPool, MockPool, SwapLine) {
        let token0 = Arc::new(Token::new_with_data(TokenAddress::WETH, Some("WETH".to_string()), None, Some(18), true, false));
        let token1 = Arc::new(Token::new_with_data(TokenAddress::USDT, Some("USDT".to_string()), None, Some(6), true, false));
        let pool1 = MockPool { token0: TokenAddress::WETH, token1: TokenAddress::USDT, address: UniswapV3PoolAddress::WETH_USDT_3000 };
        let pool2_address = Address::random();
        let pool2 = MockPool { token0: TokenAddress::WETH, token1: TokenAddress::USDT, address: UniswapV2PoolAddress::WETH_USDT };

        let swap_path =
            SwapPath::new(vec![token0.clone(), token1.clone(), token1.clone(), token0.clone()], vec![pool1.clone(), pool2.clone()]);

        let swap_line = SwapLine {
            path: swap_path,
            amount_in: SwapAmountType::Set(parse_units("0.01", "ether").unwrap().get_absolute()),
            amount_out: SwapAmountType::Set(parse_units("0.03", "ether").unwrap().get_absolute()),
            calculation_results: vec![],
            swap_to: Some(Address::default()),
            gas_used: Some(10000),
        };

        (pool1, pool2, swap_line)
    }

    #[test]
    fn test_swapline_fmt() {
        let (_, _, swap_line) = default_swap_line();

        // under test
        let formatted = format!("{}", swap_line);
        assert_eq!(
            formatted,
            "SwapLine [profit=0.02, tokens=[WETH, USDT, USDT, WETH], \
            pools=[UniswapV2@0x4e68ccd3e89f51c3074ca5072bbac773960dfa36, UniswapV2@0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852], \
            amount_in=0.01, amount_out=0.03, calculation_results=[], gas_used=Some(10000)]"
        )
    }

    #[test]
    fn test_contains_pool() {
        let (pool1, pool2, swap_line) = default_swap_line();

        assert!(swap_line.contains_pool(&PoolWrapper::from(pool1)));
        assert!(swap_line.contains_pool(&PoolWrapper::from(pool2)));
    }

    #[test]
    fn test_tokens() {
        let (_, _, swap_line) = default_swap_line();

        let tokens = swap_line.tokens();
        assert_eq!(tokens.first().unwrap().get_address(), TokenAddress::WETH);
        assert_eq!(tokens.get(1).unwrap().get_address(), TokenAddress::USDT);
    }

    #[test]
    fn test_pools() {
        let (pool1, pool2, swap_line) = default_swap_line();

        let pools = swap_line.pools();
        assert_eq!(pools.first().unwrap().get_address(), pool1.address);
        assert_eq!(pools.get(1).unwrap().get_address(), pool2.address);
    }

    #[test]
    fn test_get_first_token() {
        let (_, _, swap_line) = default_swap_line();

        let token = swap_line.get_first_token();
        assert_eq!(token.unwrap().get_address(), TokenAddress::WETH);
    }

    #[test]
    fn test_get_last_token() {
        let (_, _, swap_line) = default_swap_line();

        let token = swap_line.get_last_token();
        assert_eq!(token.unwrap().get_address(), TokenAddress::WETH);
    }

    #[test]
    fn test_get_first_pool() {
        let (pool1, _, swap_line) = default_swap_line();

        let pool = swap_line.get_first_pool();
        assert_eq!(pool.unwrap().get_address(), pool1.address);
    }

    #[test]
    fn test_get_last_pool() {
        let (_, pool2, swap_line) = default_swap_line();

        let pool = swap_line.get_last_pool();
        assert_eq!(pool.unwrap().get_address(), pool2.address);
    }
}
