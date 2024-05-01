use std::convert::Infallible;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use alloy_primitives::{Address, I256, U256};
use eyre::{eyre, Report, Result};
use lazy_static::lazy_static;
use log::{debug, trace};
use revm::InMemoryDB;
use revm::primitives::Env;

use defi_types::SwapError;

use crate::{Pool, PoolWrapper, Token};
use crate::swappath::SwapPath;

lazy_static! {
   static ref WETH_ADDRESS : Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
}


#[derive(Clone, Copy, Debug, Default)]
pub enum SwapAmountType {
    #[default]
    NotSet,
    Set(U256),
    Stack0,
    RelativeStack(u32),
    Balance(Address),
}


/*
impl TryInto<Report> for SwapError {
    type Error = Report;

    fn try_into(self) -> std::result::Result<Report, Self::Error> {
        todo!()
    }
}

 */


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
    //pub tokens: Vec<Token>,
    //pub pools: Vec<PoolWrapper>,
    pub path: SwapPath,
    pub amount_in: SwapAmountType,
    pub amount_out: SwapAmountType,
    pub amounts: Option<Vec<U256>>,
    //pub min_balance: Option<U256>,
    //pub tips: Option<U256>,
    pub swap_to: Option<Address>,
    pub gas_used: Option<u64>,
}

impl fmt::Display for SwapLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token_in = self.tokens().first().clone();
        let token_out = self.tokens().last().clone();


        let profit: String = if token_in == token_out {
            match token_in {
                Some(t) => {
                    format!("profit {}", t.to_float_sign(self.profit().unwrap_or_else(|_| I256::ZERO)))
                }
                _ => {
                    format!("profit {}", self.profit().unwrap_or_else(|_| I256::ZERO))
                }
            }
        } else {
            "".to_string()
        };


        let tokens = self.tokens().iter().map(|token| token.get_symbol()).collect::<Vec<String>>().join(", ");
        let pools = self.pools().iter().map(|pool| format!("{}@{:#20x}", pool.get_protocol(), pool.get_address())).collect::<Vec<String>>().join(", ");
        let amount_in = match self.amount_in {
            SwapAmountType::Set(x) => {
                match token_in {
                    Some(t) => {
                        format!("{:?}", t.to_float(x))
                    }
                    _ => { format!("{}", x) }
                }
            }
            _ => { format!("{:?}", self.amount_in) }
        };
        let amount_out = match self.amount_out {
            SwapAmountType::Set(x) => {
                match token_out {
                    Some(t) => {
                        format!("{:?}", t.to_float(x))
                    }
                    _ => { format!("{}", x) }
                }
            }
            _ => { format!("{:?}", self.amount_out) }
        };
        let amounts = self.amounts.as_ref().map(|amounts| {
            amounts.iter().map(|amount| amount.to_string()).collect::<Vec<String>>().join(", ")
        }).unwrap_or_else(|| "None".to_string());

        write!(
            f,
            "SwapPath [{} tokens: [{}], pools: [{}], amount_in: {}, amount_out: {}, amounts: {} ]",
            profit, tokens, pools, amount_in, amount_out, amounts
        )
    }
}

/*
impl Default for SwapLine {
    fn default() -> Self {
        SwapLine {
            //tokens : Vec::new(),
            //pools : Vec::new(),
            path : Default::default(),
            amount_in : SwapAmountType::NotSet,
            amount_out : SwapAmountType::NotSet,
            amounts : None,
            swap_to : None,
            gas_used : None,
        }
    }
}

 */

impl Hash for SwapLine {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tokens().hash(state);
        self.pools().hash(state);
    }
}

impl PartialEq for SwapLine {
    fn eq(&self, other: &Self) -> bool {
        if self.tokens() == other.tokens() && self.pools() == other.pools() {
            true
        } else {
            false
        }
    }
}

impl From<SwapPath> for SwapLine {
    fn from(value: SwapPath) -> Self {
        Self {
            path: value,
            ..Default::default()
        }
    }
}


impl SwapLine {
    pub fn new() -> Self {
        SwapLine::default()
    }

    pub fn contains_pool(&self, pool: &PoolWrapper) -> bool {
        self.path.contains_pool(pool)
    }

    pub fn tokens(&self) -> &Vec<Arc<Token>> {
        &self.path.tokens
    }

    pub fn pools(&self) -> &Vec<PoolWrapper> {
        &self.path.pools
    }


    pub fn get_first_token(&self) -> Option<&Arc<Token>> {
        self.tokens().first()
    }

    pub fn get_last_token(&self) -> Option<&Arc<Token>> {
        self.tokens().last()
    }

    pub fn get_first_pool(&self) -> Option<&PoolWrapper> {
        self.pools().first()
    }
    pub fn get_last_pool(&self) -> Option<&PoolWrapper> {
        self.pools().last()
    }


    pub fn get_token_in_address(&self) -> Option<Address> {
        let tokens = self.tokens();
        if tokens.len() == 0 {
            None
        } else {
            Some(tokens[0].get_address())
        }
    }

    pub fn is_in_token_weth(&self) -> bool {
        let tokens = self.tokens();

        if tokens.len() == 0 {
            false
        } else {
            tokens[0].get_address() == *WETH_ADDRESS
        }
    }

    pub fn split(&self, pool_index: usize) -> Result<(SwapLine, SwapLine)> {
        let first = SwapLine {
            path: SwapPath::new(self.tokens()[0..pool_index + 1].to_vec(), self.pools()[0..pool_index].to_vec()),
            amount_in: self.amount_in,
            amount_out: SwapAmountType::NotSet,
            amounts: None,
            swap_to: None,
            gas_used: None,
        };
        let second = SwapLine {
            path: SwapPath::new(self.tokens()[pool_index..].to_vec(), self.pools()[pool_index..].to_vec()),
            amount_in: SwapAmountType::NotSet,
            amount_out: self.amount_out,
            amounts: None,
            swap_to: None,
            gas_used: None,
        };
        Ok((first, second))
    }


    pub fn can_flash_swap(&self) -> bool {
        for pool in self.pools().iter() {
            if !pool.can_flash_swap() {
                return false;
            }
        }
        return true;
    }

    pub fn merge(&self, pool_index: usize) -> Result<(SwapLine, SwapLine)> {
        let first = SwapLine {
            path: SwapPath::new(self.tokens()[0..pool_index + 1].to_vec(), self.pools()[0..pool_index].to_vec()),
            amount_in: self.amount_in,
            amount_out: SwapAmountType::NotSet,
            amounts: None,
            swap_to: None,
            gas_used: None,
        };
        let second = SwapLine {
            path: SwapPath::new(self.tokens()[pool_index..].to_vec(), self.pools()[pool_index..].to_vec()),
            amount_in: SwapAmountType::NotSet,
            amount_out: self.amount_out,
            amounts: None,
            swap_to: None,
            gas_used: None,
        };
        Ok((first, second))
    }


    pub fn abs_profit(&self) -> U256 {
        if let Some(token_in) = self.tokens().first() {
            if let Some(token_out) = self.tokens().last() {
                if token_in == token_out {
                    if let SwapAmountType::Set(amount_in) = self.amount_in {
                        if let SwapAmountType::Set(amount_out) = self.amount_out {
                            if amount_out > amount_in {
                                return amount_out - amount_in;
                            }
                        }
                    }
                }
            }
        }
        U256::ZERO
    }

    pub fn abs_profit_eth(&self) -> U256 {
        let profit = self.abs_profit();
        self.get_first_token().unwrap().calc_eth_value(profit).unwrap_or(U256::ZERO)
    }


    pub fn profit(&self) -> Result<I256> {
        if self.tokens().len() < 3 {
            return Err(eyre!("NOT_ARB_PATH"));
        }
        if let Some(token_in) = self.tokens().first() {
            if let Some(token_out) = self.tokens().last() {
                if token_in == token_out {
                    if let SwapAmountType::Set(amount_in) = self.amount_in {
                        if let SwapAmountType::Set(amount_out) = self.amount_out {
                            return Ok(I256::from_raw(amount_out) - I256::from_raw(amount_in));
                        }
                    }
                    return Err(eyre!("AMOUNTS_NOT_SET"));
                } else {
                    return Err(eyre!("TOKENS_DONT_MATCH"));
                }
            }
        }
        Err(eyre!("CANNOT_CALCULATE"))
    }


    pub fn calculate_swap_path_in_amount_provided(&self, state: &InMemoryDB, env: Env, in_amount: U256) -> Result<(U256, u64), SwapError> {
        let mut out_amount = in_amount;
        let mut gas_used = 0;
        for (i, pool) in self.pools().iter().enumerate() {
            let token_from = &self.tokens()[i];
            let token_to = &self.tokens()[i + 1];
            match pool.calculate_out_amount(state, env.clone(), &token_from.get_address(), &token_to.get_address(), out_amount) {
                Ok((r, g)) => {
                    if r.is_zero() {
                        return Err(SwapError {
                            msg: "ZERO_AMOUNT".to_string(),
                            pool: pool.get_address(),
                            token_from: token_from.get_address(),
                            token_to: token_to.get_address(),
                            amount: in_amount,
                        });
                    }
                    out_amount = r;
                    gas_used += g
                }
                Err(e) => {
                    return Err(SwapError {
                        msg: e.to_string(),
                        pool: pool.get_address(),
                        token_from: token_from.get_address(),
                        token_to: token_to.get_address(),
                        amount: in_amount,
                    });
                }
            }
        }
        Ok((out_amount, gas_used))
    }

    pub fn calculate_swap_path_out_amount_provided(&self, state: &InMemoryDB, env: Env, out_amount: U256) -> Result<(U256, u64), SwapError> {
        let mut in_amount = out_amount;
        let mut gas_used = 0;
        let mut pool_reverse = self.pools().clone();
        pool_reverse.reverse();
        let mut tokens_reverse = self.tokens().clone();
        tokens_reverse.reverse();


        for (i, pool) in pool_reverse.iter().enumerate() {
            let token_from = &tokens_reverse[i + 1];
            let token_to = &tokens_reverse[i];
            match pool.calculate_in_amount(state, env.clone(), &token_from.get_address(), &token_to.get_address(), in_amount) {
                Ok((r, g)) => {
                    if r == U256::MAX || r == U256::ZERO {
                        return Err(SwapError {
                            msg: "ZERO_AMOUNT".to_string(),
                            pool: pool.get_address(),
                            token_from: token_from.get_address(),
                            token_to: token_to.get_address(),
                            amount: out_amount,
                        });
                    }
                    in_amount = r;
                    gas_used += g;
                }
                Err(e) => {
                    return Err(SwapError {
                        msg: e.to_string(),
                        pool: pool.get_address(),
                        token_from: token_from.get_address(),
                        token_to: token_to.get_address(),
                        amount: out_amount,
                    });
                }
            }
        }
        Ok((in_amount, gas_used))
    }


    fn calc_profit(in_amount: U256, out_amount: U256) -> I256 {
        I256::from_raw(out_amount) - I256::from_raw(in_amount)
    }

    pub fn optimize_swap_path_in_amount_provided(&mut self, state: &InMemoryDB, env: Env, in_amount: U256) -> Result<&mut Self, SwapError> {
        let mut current_in_amount = in_amount;
        let mut bestprofit: Option<I256> = None;
        let mut current_step = U256::from(10000);
        let mut inc_direction = true;
        let mut first_step_change = false;
        let mut next_amount = current_in_amount;
        let mut prev_in_amount = U256::ZERO;
        let mut counter = 0;
        let min_step = U256::from(100);
        let denominator = U256::from(1000);


        loop {
            counter += 1;
            //let next_amount  = current_in_amount + (current_in_amount * current_step / 10000);

            if counter > 30 {
                debug!("optimize_swap_path_in_amount iterations exceeded : {self} {current_in_amount} {current_step}");
                return Ok(self);
            }

            let current_out_amount_result = self.calculate_swap_path_in_amount_provided(state, env.clone(), next_amount);


            if counter == 1 && current_out_amount_result.is_err() {
                return Err(current_out_amount_result.err().unwrap());
            }
            let (current_out_amount, current_gas_used) = current_out_amount_result.unwrap_or_default();
            //let mut next_profit = I256::zero();


            let current_profit = Self::calc_profit(next_amount, current_out_amount);


            if bestprofit.is_none() {
                bestprofit = Some(current_profit);
                self.amount_in = SwapAmountType::Set(next_amount);
                self.amount_out = SwapAmountType::Set(current_out_amount);
                self.gas_used = Some(current_gas_used);
                current_in_amount = next_amount;
                if current_out_amount.is_zero() || current_profit.is_negative() {
                    return Ok(self);
                }
            } else {
                if bestprofit.unwrap() > current_profit || current_out_amount.is_zero() /*|| next_profit < current_profit*/ {
                    if first_step_change && inc_direction && current_step < denominator {
                        inc_direction = false;
                        next_amount = prev_in_amount;
                        current_in_amount = prev_in_amount;
                        first_step_change = true;
                        //debug!("inc direction changed {} {} {}", next_amount, current_profit, bestprofit.unwrap());
                    } else if first_step_change && !inc_direction {
                        current_in_amount = current_in_amount;
                        inc_direction = true;
                        current_step /= U256::from(10);
                        bestprofit = Some(current_profit);
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
                    bestprofit = Some(current_profit);
                    self.amount_in = SwapAmountType::Set(next_amount);
                    self.amount_out = SwapAmountType::Set(current_out_amount);
                    self.gas_used = Some(current_gas_used);
                    current_in_amount = next_amount;
                    first_step_change = false;
                }
            }


            prev_in_amount = current_in_amount;
            if inc_direction {
                next_amount = current_in_amount + (current_in_amount * current_step / denominator);
            } else {
                next_amount = current_in_amount - (current_in_amount * current_step / denominator);
            }
            //debug!("opt step : {} direction {} first_step {} step : {} current_in_amount : {} next_amount: {} profit : {} {}", counter, inc_direction, first_step_change,  current_step, current_in_amount , next_amount, current_profit, bestprofit.unwrap());
        }


        Ok(self)
    }
}

