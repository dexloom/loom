use std::fmt::{Display, Formatter};
use std::sync::Arc;

use alloy_primitives::{I256, U256};
use eyre::{eyre, ErrReport, Result};
use revm::primitives::Env;
use revm::DatabaseRef;
use tracing::error;

use crate::{PoolWrapper, SwapAmountType, SwapLine, Token};
use loom_evm_db::LoomDBType;
use loom_types_blockchain::LoomDataTypes;

#[derive(Clone, Debug)]
pub struct SwapStep<LDT: LoomDataTypes> {
    swap_line_vec: Vec<SwapLine<LDT>>,
    swap_from: Option<LDT::Address>,
    swap_to: LDT::Address,
}

impl<LDT: LoomDataTypes> Display for SwapStep<LDT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let paths = self.swap_line_vec.iter().map(|path| format!("{path}")).collect::<Vec<String>>().join(" / ");
        write!(f, "{}", paths)
    }
}

impl<LDT: LoomDataTypes> SwapStep<LDT> {
    pub fn new(swap_to: LDT::Address) -> Self {
        Self { swap_line_vec: Vec::new(), swap_to, swap_from: None }
    }

    pub fn get_mut_swap_line_by_index(&mut self, idx: usize) -> &mut SwapLine<LDT> {
        &mut self.swap_line_vec[idx]
    }

    pub fn swap_line_vec(&self) -> &Vec<SwapLine<LDT>> {
        &self.swap_line_vec
    }

    pub fn len(&self) -> usize {
        self.swap_line_vec.len()
    }

    pub fn is_empty(&self) -> bool {
        self.swap_line_vec.is_empty()
    }

    fn first_swap_line(&self) -> Option<&SwapLine<LDT>> {
        self.swap_line_vec.first()
    }

    pub fn first_token(&self) -> Option<&Arc<Token<LDT>>> {
        match self.first_swap_line() {
            Some(s) => s.get_first_token(),
            None => None,
        }
    }

    pub fn last_token(&self) -> Option<&Arc<Token<LDT>>> {
        match self.first_swap_line() {
            Some(s) => s.get_last_token(),
            None => None,
        }
    }

    pub fn add(&mut self, swap_path: SwapLine<LDT>) -> &mut Self {
        if self.is_empty()
            || ((self.first_token().unwrap() == swap_path.get_first_token().unwrap())
                && (self.last_token().unwrap() == swap_path.get_last_token().unwrap()))
        {
            self.swap_line_vec.push(swap_path);
        } else {
            error!(
                "cannot add SwapPath {} != {} {} !=  {}",
                self.first_token().unwrap().get_address(),
                swap_path.get_first_token().unwrap().get_address(),
                self.last_token().unwrap().get_address(),
                swap_path.get_last_token().unwrap().get_address()
            )
        }
        self
    }

    pub fn can_flash_swap(&self) -> bool {
        for swap_line in self.swap_line_vec.iter() {
            for pool in swap_line.pools().iter() {
                if !pool.can_flash_swap() {
                    return false;
                }
            }
        }
        true
    }

    pub fn can_calculate_in_amount(&self) -> bool {
        for swap_line in self.swap_line_vec.iter() {
            for pool in swap_line.pools().iter() {
                if !pool.can_calculate_in_amount() {
                    return false;
                }
            }
        }
        true
    }

    pub fn get_pools(&self) -> Vec<PoolWrapper<LDT>> {
        self.swap_line_vec.iter().flat_map(|sp| sp.pools().clone()).collect()
    }

    fn common_pools(swap_path_0: &SwapLine<LDT>, swap_path_1: &SwapLine<LDT>) -> usize {
        let mut ret = 0;
        for pool in swap_path_0.pools().iter() {
            if swap_path_1.pools().contains(pool) {
                ret += 1;
            }
        }
        ret
    }

    pub fn merge_swap_paths(
        swap_path_0: SwapLine<LDT>,
        swap_path_1: SwapLine<LDT>,
        multicaller: LDT::Address,
    ) -> Result<(SwapStep<LDT>, SwapStep<LDT>)> {
        let mut split_index_start = 0;
        let mut split_index_end = 0;

        if swap_path_0.get_first_token().unwrap() != swap_path_1.get_first_token().unwrap()
            || swap_path_0.get_last_token().unwrap() != swap_path_1.get_last_token().unwrap()
        {
            return Err(eyre!("CANNOT_MERGE_DIFFERENT_TOKENS"));
        }

        for i in 0..swap_path_0.pools().len() {
            if i >= swap_path_1.pools().len() {
                break;
            }
            let pool_0 = &swap_path_0.pools()[i];
            let pool_1 = &swap_path_1.pools()[i];

            let token_0 = &swap_path_0.tokens()[i + 1];
            let token_1 = &swap_path_1.tokens()[i + 1];

            if pool_0 == pool_1 && token_0 == token_1 {
                split_index_start += 1;
            } else {
                break;
            }
        }

        for i in 0..swap_path_0.pools().len() {
            if i >= swap_path_1.pools().len() {
                break;
            }
            let pool_0 = &swap_path_0.pools()[swap_path_0.pools().len() - 1 - i];
            let pool_1 = &swap_path_1.pools()[swap_path_1.pools().len() - 1 - i];

            let token_0 = &swap_path_0.tokens()[swap_path_0.tokens().len() - 2 - i];
            let token_1 = &swap_path_1.tokens()[swap_path_1.tokens().len() - 2 - i];

            if pool_0 == pool_1 && token_0 == token_1 {
                split_index_end += 1;
            } else {
                break;
            }
        }

        if split_index_start > 0 && split_index_end > 0 {
            return Err(eyre!("CANNOT_MERGE_BOTH_SIDES"));
        }

        let common_pools_count = Self::common_pools(&swap_path_0, &swap_path_1);
        if (split_index_start > 0 && split_index_start != common_pools_count)
            || (split_index_end > 0 && split_index_end != common_pools_count)
        {
            return Err(eyre!("MORE_COMMON_POOLS"));
        }

        if split_index_start > 0 {
            let (mut split_0_0, mut split_0_1) = swap_path_0.split(split_index_start)?;
            let (split_1_0, mut split_1_1) = swap_path_1.split(split_index_start)?;

            let mut swap_step_0 = SwapStep::<LDT>::new(multicaller);
            let mut swap_step_1 = SwapStep::<LDT>::new(multicaller);

            if let SwapAmountType::Set(a0) = swap_path_0.amount_in {
                if let SwapAmountType::Set(a1) = swap_path_1.amount_in {
                    split_0_0.amount_in = SwapAmountType::Set(a0.max(a1) >> 1);
                }
            }

            /*if let SwapAmountType::Set(a0) = swap_path_0.amount_out {
                if let SwapAmountType::Set(a1) = swap_path_1.amount_out {
                    split_0_0.amount_out = SwapAmountType::Set(a0.max(a1));
                }
            }*/

            /*
            if let SwapAmountType::Set(a0) = swap_path_0.amount_in {
                if let SwapAmountType::Set(a1) = swap_path_1.amount_in {
                    split_0_1.amount_in = SwapAmountType::Set(a0 / (a0 + a1));
                    split_1_1.amount_in = SwapAmountType::Set(a1 / (a0 + a1));
                }
            }

             */

            if let SwapAmountType::Set(a0) = swap_path_0.amount_out {
                if let SwapAmountType::Set(a1) = swap_path_1.amount_out {
                    split_0_1.amount_out = SwapAmountType::Set((a0.max(a1) * a0 / (a0 + a1)) >> 1);
                    split_1_1.amount_out = SwapAmountType::Set((a0.max(a1) * a1 / (a0 + a1)) >> 1);
                }
            }

            swap_step_0.add(split_0_0);

            swap_step_1.add(split_0_1);
            swap_step_1.add(split_1_1);

            return Ok((swap_step_0, swap_step_1));
        }

        if split_index_end > 0 {
            let (mut split_0_0, mut split_0_1) = swap_path_0.split(swap_path_0.pools().len() - split_index_end)?;
            let (mut split_1_0, split_1_1) = swap_path_1.split(swap_path_1.pools().len() - split_index_end)?;

            let mut swap_step_0 = SwapStep::new(multicaller);
            let mut swap_step_1 = SwapStep::new(multicaller);

            if let SwapAmountType::Set(a0) = swap_path_0.amount_out {
                if let SwapAmountType::Set(a1) = swap_path_1.amount_out {
                    split_0_1.amount_out = SwapAmountType::Set(a0.max(a1) >> 1);
                }
            }

            /*
            if let SwapAmountType::Set(a0) = swap_path_0.amount_in {
                if let SwapAmountType::Set(a1) = swap_path_1.amount_in {
                    split_0_1.amount_in = SwapAmountType::Set(a0.max(a1))
                }
            }

             */

            if let SwapAmountType::Set(a0) = swap_path_0.amount_in {
                if let SwapAmountType::Set(a1) = swap_path_1.amount_in {
                    split_0_0.amount_in = SwapAmountType::Set((a0.max(a1) * a0 / (a0 + a1)) >> 1);
                    split_1_0.amount_in = SwapAmountType::Set((a0.max(a1) * a1 / (a0 + a1)) >> 1);
                }
            }

            /*
            if let SwapAmountType::Set(a0) = swap_path_0.amount_out {
                if let SwapAmountType::Set(a1) = swap_path_1.amount_out {
                    split_0_0.amount_out = SwapAmountType::Set(a0 );
                    split_1_0.amount_out = SwapAmountType::Set(a1 );
                }
            }

             */

            swap_step_0.add(split_0_0);
            swap_step_0.add(split_1_0);

            swap_step_1.add(split_0_1);

            return Ok((swap_step_0, swap_step_1));
        }

        Err(eyre!("CANNOT_MERGE"))
    }

    pub fn get_first_token_address(&self) -> Option<LDT::Address> {
        let mut ret: Option<LDT::Address> = None;
        for sp in self.swap_line_vec.iter() {
            match sp.get_first_token() {
                Some(token) => match ret {
                    Some(a) => {
                        if a != token.get_address() {
                            return None;
                        }
                    }
                    None => {
                        ret = Some(token.get_address());
                    }
                },
                _ => {
                    return None;
                }
            }
        }
        ret
    }

    pub fn get_first_pool(&self) -> Option<&PoolWrapper<LDT>> {
        if self.swap_line_vec.len() == 1 {
            self.swap_line_vec.first().and_then(|x| x.path.pools.first())
        } else {
            None
        }
    }

    pub fn get_last_pool(&self) -> Option<&PoolWrapper<LDT>> {
        if self.swap_line_vec.len() == 1 {
            self.swap_line_vec.first().and_then(|x| x.path.pools.last())
        } else {
            None
        }
    }

    pub fn get_first_token(&self) -> Option<&Arc<Token<LDT>>> {
        let mut ret: Option<&Arc<Token<LDT>>> = None;
        for sp in self.swap_line_vec.iter() {
            match sp.get_first_token() {
                Some(token) => match &ret {
                    Some(a) => {
                        if a.get_address() != token.get_address() {
                            return None;
                        }
                    }
                    None => {
                        ret = Some(token);
                    }
                },
                _ => {
                    return None;
                }
            }
        }
        ret
    }

    pub fn get_in_amount(&self) -> Result<U256> {
        let mut in_amount = U256::ZERO;
        for swap_path in self.swap_line_vec.iter() {
            match swap_path.amount_in {
                SwapAmountType::Set(amount) => in_amount += amount,
                _ => return Err(eyre!("IN_AMOUNT_NOT_SET")),
            }
        }
        Ok(in_amount)
    }

    pub fn get_out_amount(&self) -> Result<U256> {
        let mut out_amount = U256::ZERO;
        for swap_path in self.swap_line_vec.iter() {
            match swap_path.amount_out {
                SwapAmountType::Set(amount) => out_amount += amount,
                _ => return Err(eyre!("IN_AMOUNT_NOT_SET")),
            }
        }
        Ok(out_amount)
    }

    pub fn calculate_with_in_amount<DB: DatabaseRef<Error = ErrReport>>(
        &mut self,
        state: &DB,
        env: Env,
        in_ammount: Option<U256>,
    ) -> Result<(U256, u64)> {
        let mut out_amount = U256::ZERO;
        let mut gas_used = 0;

        for swap_path in self.swap_line_vec.iter_mut() {
            let cur_in_amount = match in_ammount {
                Some(amount) => amount,
                None => match swap_path.amount_in {
                    SwapAmountType::Set(amount) => amount,
                    _ => {
                        return Err(eyre!("IN_AMOUNT_NOT_SET"));
                    }
                },
            };
            swap_path.amount_in = SwapAmountType::Set(cur_in_amount);

            match swap_path.calculate_with_in_amount(state, env.clone(), cur_in_amount) {
                Ok((amount, gas, calculation_results)) => {
                    out_amount += amount;
                    swap_path.amount_out = SwapAmountType::Set(amount);
                    gas_used += gas;
                }
                _ => {
                    return Err(eyre!("ERROR_CALCULATING_OUT_AMOUNT"));
                }
            }
        }
        Ok((out_amount, gas_used))
    }

    pub fn calculate_with_out_amount(&mut self, state: &LoomDBType, env: Env, out_amount: Option<U256>) -> Result<(U256, u64)> {
        let mut in_amount = U256::ZERO;
        let mut gas_used = 0;

        for swap_path in self.swap_line_vec.iter_mut() {
            let cur_out_amount = match out_amount {
                Some(amount) => amount,
                None => match swap_path.amount_out {
                    SwapAmountType::Set(amount) => amount,
                    _ => {
                        return Err(eyre!("IN_AMOUNT_NOT_SET"));
                    }
                },
            };

            swap_path.amount_out = SwapAmountType::Set(cur_out_amount);

            match swap_path.calculate_with_out_amount(state, env.clone(), cur_out_amount) {
                Ok((amount, gas, calculation_results)) => {
                    in_amount += amount;
                    gas_used += gas;
                    swap_path.amount_in = SwapAmountType::Set(amount);
                }
                _ => {
                    return Err(eyre!("ERROR_CALCULATING_OUT_AMOUNT"));
                }
            }
        }
        Ok((in_amount, gas_used))
    }

    /*

    fn optimize_swap_step_in_amount_provided(&mut self, state: &dyn DatabaseRef<Error=Infallible>, env: Env, step : I256, ) -> Result<Self> {
        let best_idx = Option<usize>;
        let best_out_amount = Option<usize>;
        let cu_out_amount = self.get_out_amount().unwrap_or(U256::zero());

        for (idx, swap_path) in self.swap_path_vec.iter_mut().enumerate() {
            match swap_path.amount_in {
                InAmountType::Set(amount) {
                    swap_path.calculate_swap_path_in_amount_provided(state, env.clone(), )
                }
                _=>{ return Err(eyre!("IN_AMOUNT_NOT_SET"))}

            }
        }


    }*/

    pub fn profit(swap_step_0: &SwapStep<LDT>, swap_step_1: &SwapStep<LDT>) -> I256 {
        let in_amount: I256 = I256::try_from(swap_step_0.get_in_amount().unwrap_or(U256::MAX)).unwrap_or(I256::MAX);
        let out_amount: I256 = I256::try_from(swap_step_1.get_out_amount().unwrap_or(U256::ZERO)).unwrap_or(I256::ZERO);
        if in_amount.is_negative() {
            I256::MIN
        } else {
            out_amount - in_amount
        }
    }

    pub fn abs_profit(swap_step_0: &SwapStep<LDT>, swap_step_1: &SwapStep<LDT>) -> U256 {
        let in_amount: U256 = swap_step_0.get_in_amount().unwrap_or(U256::MAX);
        let out_amount: U256 = swap_step_1.get_out_amount().unwrap_or(U256::ZERO);
        if in_amount >= out_amount {
            U256::ZERO
        } else {
            out_amount - in_amount
        }
    }

    pub fn abs_profit_eth(swap_step_0: &SwapStep<LDT>, swap_step_1: &SwapStep<LDT>) -> U256 {
        match swap_step_0.get_first_token() {
            Some(t) => {
                let profit = Self::abs_profit(swap_step_0, swap_step_1);
                t.calc_eth_value(profit).unwrap_or_default()
            }
            _ => U256::ZERO,
        }
    }

    pub fn optimize_swap_steps<DB: DatabaseRef<Error = ErrReport>>(
        state: &DB,
        env: Env,
        swap_step_0: &SwapStep<LDT>,
        swap_step_1: &SwapStep<LDT>,
        middle_amount: Option<U256>,
    ) -> Result<(SwapStep<LDT>, SwapStep<LDT>)> {
        if swap_step_0.can_calculate_in_amount() {
            SwapStep::optimize_with_middle_amount(state, env, swap_step_0, swap_step_1, middle_amount)
        } else {
            SwapStep::optimize_with_in_amount(state, env, swap_step_0, swap_step_1, middle_amount)
        }
    }

    pub fn optimize_with_middle_amount<DB: DatabaseRef<Error = ErrReport>>(
        state: &DB,
        env: Env,
        swap_step_0: &SwapStep<LDT>,
        swap_step_1: &SwapStep<LDT>,
        middle_amount: Option<U256>,
    ) -> Result<(SwapStep<LDT>, SwapStep<LDT>)> {
        let mut step_0 = swap_step_0.clone();
        let mut step_1 = swap_step_1.clone();
        let mut best_profit: Option<I256> = None;

        let (middle_amount, _) = match middle_amount {
            Some(amount) => step_0.calculate_with_in_amount(state, env.clone(), Some(amount))?,
            _ => step_0.calculate_with_in_amount(state, env.clone(), None)?,
        };

        let step_0_out_amount = step_0.get_out_amount()?;
        let step_1_out_amount = step_1.get_out_amount()?;

        for swap_path_1 in step_1.swap_line_vec.iter_mut() {
            let in_amount = step_0_out_amount * swap_path_1.amount_out.unwrap() / step_1_out_amount;
            swap_path_1.amount_in = SwapAmountType::Set(in_amount);
        }

        step_1.calculate_with_in_amount(state, env.clone(), None);

        let cur_profit = Self::profit(&step_0, &step_1);
        if cur_profit.is_positive() {
            best_profit = Some(cur_profit);
        }

        //if step_0.get_in_amount()? > step_1.get_out_amount()? {
        //    return Ok((step_0, step_1))
        //}

        //let step_0_in_amount = step_0.get_in_amount().unwrap_or(U256::max_value());
        //let step_1_out_amount = step_1.get_out_amount().unwrap_or(U256::zero());

        let denominator = U256::from(10000);
        let step_multiplier = U256::from(500);

        let mut step_0_calc = step_0.clone();
        let mut step_1_calc = step_1.clone();

        let mut counter = 0;

        let step = middle_amount * step_multiplier / denominator;

        loop {
            counter += 1;
            if counter > 30 {
                return if Self::profit(&step_0, &step_1).is_positive() { Ok((step_0, step_1)) } else { Err(eyre!("TOO_MANY_STEPS")) };
            }

            let step0in = step_0.get_in_amount()?;
            let step0out = step_0.get_out_amount()?;
            let step1in = step_1.get_in_amount()?;
            let step1out = step_1.get_out_amount()?;
            let profit = Self::profit(&step_0, &step_1);

            //debug!("middle_amount Steps :  {} in {} out {} in {} out {} profit {}", counter, step0in, step0out, step1in, step1out, profit);

            for (i, swap_path_0_calc) in step_0_calc.swap_line_vec.iter_mut().enumerate() {
                let amount_out = step_0.swap_line_vec[i].amount_out.unwrap();

                if amount_out <= step {
                    return Ok((step_0, step_1));
                }

                let new_out_amount = amount_out.checked_add(step);
                match new_out_amount {
                    Some(new_out_amount) => {
                        if swap_path_0_calc.amount_out.unwrap() != new_out_amount {
                            let new_amount_out = amount_out + step;
                            let (in_amount, gas, calculation_results) = swap_path_0_calc
                                .calculate_with_out_amount(state, env.clone(), new_amount_out)
                                .unwrap_or((U256::MAX, 0, vec![]));
                            swap_path_0_calc.amount_in = SwapAmountType::Set(in_amount);
                            swap_path_0_calc.amount_out = SwapAmountType::Set(new_amount_out);
                            swap_path_0_calc.gas_used = Some(gas);
                            swap_path_0_calc.calculation_results = calculation_results;
                        }
                    }
                    None => {
                        swap_path_0_calc.amount_in = SwapAmountType::Set(U256::MAX);
                    }
                }
            }

            for (i, swap_path_1_calc) in step_1_calc.swap_line_vec.iter_mut().enumerate() {
                let amount_in = step_1.swap_line_vec[i].amount_in.unwrap();

                if amount_in <= step || amount_in == U256::MAX {
                    return Ok((step_0, step_1));
                }

                let new_amount_in = amount_in.checked_add(step);

                match new_amount_in {
                    Some(new_amount_in) => {
                        if swap_path_1_calc.amount_in.unwrap() != new_amount_in {
                            let (out_amount, gas, calculation_results) = swap_path_1_calc
                                .calculate_with_in_amount(state, env.clone(), new_amount_in)
                                .unwrap_or((U256::ZERO, 0, vec![]));
                            swap_path_1_calc.amount_out = SwapAmountType::Set(out_amount);
                            swap_path_1_calc.amount_in = SwapAmountType::Set(new_amount_in);
                            swap_path_1_calc.gas_used = Some(gas);
                        }
                    }
                    None => {
                        swap_path_1_calc.amount_out = SwapAmountType::Set(U256::ZERO);
                    }
                }
            }

            let mut best_merged_step_0: Option<SwapStep<LDT>> = None;

            for i in 0..step_0.swap_line_vec.len() {
                let mut merged_step_0 = SwapStep::new(step_0.swap_to);
                for ci in 0..step_0.swap_line_vec.len() {
                    merged_step_0.add(if ci == i { step_0_calc.swap_line_vec[ci].clone() } else { step_0.swap_line_vec[ci].clone() });
                }
                if best_merged_step_0.is_none() || best_merged_step_0.clone().unwrap().get_in_amount()? > merged_step_0.get_in_amount()? {
                    best_merged_step_0 = Some(merged_step_0);
                }
            }

            let mut best_merged_step_1: Option<SwapStep<LDT>> = None;

            for i in 0..step_1.swap_line_vec.len() {
                let mut merged_step_1 = SwapStep::new(step_1.swap_to);
                for ci in 0..step_1.swap_line_vec.len() {
                    merged_step_1.add(if ci == i { step_1_calc.swap_line_vec[ci].clone() } else { step_1.swap_line_vec[ci].clone() });
                }
                if best_merged_step_1.is_none() || best_merged_step_1.clone().unwrap().get_out_amount()? < merged_step_1.get_out_amount()? {
                    best_merged_step_1 = Some(merged_step_1);
                }
            }

            //let new_middle_amount = middle_amount - step;

            if best_merged_step_0.is_none() || best_merged_step_1.is_none() {
                //debug!("optimize_swap_steps_middle_amount {} {}", counter, Self::profit(&step_0, &step_1)  );
                return if Self::profit(&step_0, &step_1).is_positive() {
                    Ok((step_0, step_1))
                } else {
                    Err(eyre!("CANNOT_OPTIMIZE_SWAP_STEP"))
                };
            }

            let best_merged_step_0 = best_merged_step_0.unwrap();
            let best_merged_step_1 = best_merged_step_1.unwrap();

            let cur_profit = Self::profit(&best_merged_step_0, &best_merged_step_1);

            if best_profit.is_none() || best_profit.unwrap() < cur_profit {
                step_0 = best_merged_step_0;
                step_1 = best_merged_step_1;
                best_profit = Some(cur_profit);
            } else {
                //debug!("optimize_swap_steps_middle_amount {} {} {}", counter, Self::profit(&step_0, &step_1), Self::profit(&best_merged_step_0, &best_merged_step_1)  );
                return if Self::profit(&step_0, &step_1).is_positive() {
                    Ok((step_0, step_1))
                } else {
                    Err(eyre!("CANNOT_OPTIMIZE_SWAP_STEP"))
                };
            }
        }
    }

    pub fn optimize_with_in_amount<DB: DatabaseRef<Error = ErrReport>>(
        state: &DB,
        env: Env,
        swap_step_0: &SwapStep<LDT>,
        swap_step_1: &SwapStep<LDT>,
        in_amount: Option<U256>,
    ) -> Result<(SwapStep<LDT>, SwapStep<LDT>)> {
        let mut step_0 = swap_step_0.clone();
        let mut step_1 = swap_step_1.clone();
        let mut best_profit: Option<I256> = None;

        match in_amount {
            Some(amount) => step_0.calculate_with_in_amount(state, env.clone(), Some(amount))?,
            _ => step_0.calculate_with_in_amount(state, env.clone(), None)?,
        };
        let in_amount = step_0.get_in_amount()?;

        let step_0_out_amount = step_0.get_out_amount()?;
        let step_1_out_amount = step_1.get_out_amount()?;

        for swap_path_1 in step_1.swap_line_vec.iter_mut() {
            let in_amount = step_0_out_amount * swap_path_1.amount_out.unwrap() / step_1_out_amount;
            swap_path_1.amount_in = SwapAmountType::Set(in_amount);
        }
        let _ = step_1.calculate_with_in_amount(state, env.clone(), None)?;

        //debug!("AfterCalc SwapStep0 {:?}", step_0);
        //debug!("AfterCalc SwapStep1 {:?}", step_1);

        let cur_profit = Self::profit(&step_0, &step_1);
        if cur_profit.is_positive() {
            best_profit = Some(cur_profit);
        }

        /*if step_0.get_in_amount()? > step_1.get_out_amount()? {
            return Ok((step_0, step_1))
        }
         */

        let step_0_in_amount = step_0.get_in_amount().unwrap_or(U256::MAX);
        let step_1_out_amount = step_1.get_out_amount().unwrap_or(U256::ZERO);

        let denominator = U256::from(10000);
        let step_multiplier = U256::from(500);

        let mut step_0_calc = step_0.clone();
        let mut step_1_calc = step_1.clone();

        let mut counter = 0;

        let step = in_amount * step_multiplier / denominator;

        loop {
            counter += 1;
            if counter > 30 {
                return if Self::profit(&step_0, &step_1).is_positive() { Ok((step_0, step_1)) } else { Err(eyre!("TOO_MANY_STEPS")) };
            }

            let step0in = step_0.get_in_amount()?;
            let step0out = step_0.get_out_amount()?;
            let step1in = step_1.get_in_amount()?;
            let step1out = step_1.get_out_amount()?;
            let profit = Self::profit(&step_0, &step_1);

            //debug!("in_amount Steps :  {} in {} out {} in {} out {} profit {}", counter, step0in, step0out, step1in, step1out, profit);

            for (i, swap_path_0_calc) in step_0_calc.swap_line_vec.iter_mut().enumerate() {
                if step_0.swap_line_vec[i].amount_in.unwrap() > step
                    && swap_path_0_calc.amount_in.unwrap() != step_0.swap_line_vec[i].amount_in.unwrap() - step
                {
                    let new_amount_in = step_0.swap_line_vec[i].amount_in.unwrap() + step;
                    let (amount_out, gas, calculation_results) =
                        swap_path_0_calc.calculate_with_in_amount(state, env.clone(), new_amount_in).unwrap_or((U256::ZERO, 0, vec![]));
                    swap_path_0_calc.amount_in = SwapAmountType::Set(new_amount_in);
                    swap_path_0_calc.amount_out = SwapAmountType::Set(amount_out);
                    swap_path_0_calc.gas_used = Some(gas);
                    swap_path_0_calc.calculation_results = calculation_results;
                }
            }

            let mut best_merged_step_0: Option<SwapStep<LDT>> = None;

            for i in 0..step_0.swap_line_vec.len() {
                let mut merged_step_0 = SwapStep::new(step_0.swap_to);
                for ci in 0..step_0.swap_line_vec.len() {
                    merged_step_0.add(if ci == i { step_0_calc.swap_line_vec[ci].clone() } else { step_0.swap_line_vec[ci].clone() });
                }
                if step_0.get_in_amount()? < step || merged_step_0.get_in_amount()? != step_0.get_in_amount()? + step {
                    //error!("{:?} {} {:?}", step_0.get_in_amount(), step , merged_step_0.get_in_amount() );
                    continue;
                }

                if best_merged_step_0.is_none() || best_merged_step_0.clone().unwrap().get_out_amount()? < merged_step_0.get_out_amount()? {
                    best_merged_step_0 = Some(merged_step_0);
                }
            }

            if best_merged_step_0.is_none() {
                //error!("optimize_swap_steps_in_amount best merged step is None {}", counter );
                break;
            };

            let middle_amount_step = best_merged_step_0.clone().unwrap().get_out_amount()? - step_1.get_in_amount()?;

            for (i, swap_path_1_calc) in step_1_calc.swap_line_vec.iter_mut().enumerate() {
                if step_1.swap_line_vec[i].amount_in.unwrap() > middle_amount_step
                    && swap_path_1_calc.amount_in.unwrap() != step_1.swap_line_vec[i].amount_in.unwrap() - middle_amount_step
                {
                    let new_amount_in = step_1.swap_line_vec[i].amount_in.unwrap() + middle_amount_step;
                    let (out_amount, gas, calculation_results) =
                        swap_path_1_calc.calculate_with_in_amount(state, env.clone(), new_amount_in).unwrap_or_default();
                    swap_path_1_calc.amount_out = SwapAmountType::Set(out_amount);
                    swap_path_1_calc.amount_in = SwapAmountType::Set(new_amount_in);
                    swap_path_1_calc.gas_used = Some(gas);
                    swap_path_1_calc.calculation_results = calculation_results;
                }
            }

            let mut best_merged_step_1: Option<SwapStep<LDT>> = None;

            for i in 0..step_1.swap_line_vec.len() {
                let mut merged_step_1 = SwapStep::new(step_1.swap_to);
                for ci in 0..step_1.swap_line_vec.len() {
                    merged_step_1.add(if ci == i { step_1_calc.swap_line_vec[ci].clone() } else { step_1.swap_line_vec[ci].clone() });
                }
                if merged_step_1.get_in_amount()? != best_merged_step_0.clone().unwrap().get_out_amount()? {
                    continue;
                }

                if best_merged_step_1.is_none() || best_merged_step_1.clone().unwrap().get_out_amount()? < merged_step_1.get_out_amount()? {
                    best_merged_step_1 = Some(merged_step_1);
                }
            }

            //let new_in_amount = middle_amount - step;

            if best_merged_step_0.is_none() || best_merged_step_1.is_none() {
                //debug!("optimize_swap_steps_in_amount {} {}", counter, Self::profit(&step_0, &step_1)  );

                return if Self::profit(&step_0, &step_1).is_positive() {
                    Ok((step_0, step_1))
                } else {
                    //continue
                    Err(eyre!("CANNOT_OPTIMIZE_SWAP_STEP"))
                };
            }
            let best_merged_step_0 = best_merged_step_0.unwrap();
            let best_merged_step_1 = best_merged_step_1.unwrap();

            let cur_profit = Self::profit(&best_merged_step_0, &best_merged_step_1);

            if best_profit.is_none() || best_profit.unwrap() < cur_profit {
                step_0 = best_merged_step_0;
                step_1 = best_merged_step_1;
                best_profit = Some(cur_profit);
            } else {
                //debug!("optimize_swap_steps_in_amount {} {} {}", counter, Self::profit(&step_0, &step_1), Self::profit(&best_merged_step_0, &best_merged_step_1)  );

                return if Self::profit(&step_0, &step_1).is_positive() {
                    Ok((step_0, step_1))
                } else {
                    Err(eyre!("CANNOT_OPTIMIZE_SWAP_STEP"))
                };
            }
        }

        if Self::profit(&step_0, &step_1).is_positive() {
            Ok((step_0, step_1))
        } else {
            Err(eyre!("OPTIMIZATION_FAILED"))
        }
    }
}
