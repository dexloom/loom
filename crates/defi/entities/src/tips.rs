use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::{Swap, Token};
use alloy_primitives::utils::format_units;
use alloy_primitives::{Address, U256};
use eyre::{eyre, OptionExt, Result};
use lazy_static::lazy_static;
use loom_evm_utils::NWETH;
use rand::random;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub struct Tips {
    pub token_in: Arc<Token>,
    pub profit: U256,
    pub profit_eth: U256,
    pub tips: U256,
    pub min_change: U256,
}

impl Display for Tips {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} : tips {} min_change {} profit : {} eth : {} ",
            self.token_in.get_symbol(),
            format_units(self.tips, "ether").unwrap_or_default(),
            self.token_in.to_float(self.min_change),
            self.token_in.to_float(self.profit),
            format_units(self.profit_eth, "ether").unwrap_or_default(),
        )
    }
}

lazy_static! {
    static ref START_PCT : U256 = U256::from(9900);

    static ref SLOPES : Vec<(U256,U256)> = vec![
//x        (U256::from(0), U256::from(10000)),
        (U256::from(10).pow(U256::from(19)), U256::from(7000)),
        (U256::from(10).pow(U256::from(19))*U256::from(5), U256::from(5000))
    ];
}
pub fn tips_pct_advanced(profit: &U256) -> u32 {
    let mut start_point = U256::ZERO;
    let mut start_pct = *START_PCT;
    for (x, y) in SLOPES.iter() {
        if x > profit {
            return (start_pct - ((start_pct - y) * (profit - start_point) / (x - start_point))).to::<u32>();
        }
        start_point = *x;
        start_pct = *y;
    }
    start_pct.to()
}

pub fn randomize_tips_pct(tips_pct: u32) -> u32 {
    let rnd: u32 = random::<u32>() % 50;
    tips_pct - rnd
}

pub fn tips_and_value_for_swap_type(
    swap: &Swap,
    tips_pct: Option<u32>,
    gas_cost: Option<U256>,
    eth_balance: U256,
) -> Result<(Vec<Tips>, U256)> {
    let total_profit_eth = swap.abs_profit_eth();
    info!("Total profit eth : {}", format_units(total_profit_eth, "ether").unwrap_or_default());
    let tips_pct = randomize_tips_pct(tips_pct.unwrap_or(tips_pct_advanced(&total_profit_eth)));

    if let Some(gas_cost) = gas_cost {
        if total_profit_eth < gas_cost {
            info!(
                "total_profit_eth={} < {}",
                format_units(total_profit_eth, "ether").unwrap_or_default(),
                format_units(gas_cost, "ether").unwrap_or_default()
            );
            return Err(eyre!("NOT_ENOUGH_PROFIT"));
        }
    }

    match swap {
        Swap::BackrunSwapLine(_) | Swap::BackrunSwapSteps(_) => {
            let profit = swap.abs_profit();
            if profit.is_zero() {
                error!(profit = NWETH::to_float(profit), %swap, "Zero profit");
                return Err(eyre!("NO_PROFIT"));
            }
            let token_in = swap.get_first_token().ok_or_eyre("NO_FIRST_TOKEN")?.clone();
            let profit_eth = token_in.calc_eth_value(profit).ok_or_eyre("CALC_ETH_VALUE_FAILED")?;

            if let Some(gas_cost) = gas_cost {
                if profit_eth < gas_cost {
                    error!(
                        profit_eth = NWETH::to_float(profit_eth),
                        gas_cost = NWETH::to_float(gas_cost),
                        %swap,
                        "Profit doesn't exceed the gas cost"
                    );
                    return Err(eyre!("NO_PROFIT_EXCEEDING_GAS"));
                }
            }

            let mut tips = profit_eth.checked_sub(gas_cost.unwrap_or_default()).ok_or_eyre("SUBTRACTION_OVERFLOWN")? * U256::from(tips_pct)
                / U256::from(10000);
            let min_change = token_in.calc_token_value_from_eth(gas_cost.unwrap_or_default() + tips).unwrap();
            let mut value = if token_in.is_weth() { U256::ZERO } else { tips };

            if !token_in.is_weth() && (tips > ((eth_balance * U256::from(9000)) / U256::from(10000))) {
                tips = (eth_balance * U256::from(9000)) / U256::from(10000);
                value = tips;
            }

            Ok((vec![Tips { token_in, profit, profit_eth, tips, min_change }], value))
        }
        Swap::Multiple(swap_vec) => {
            let mut tips_hashset: HashMap<Address, Tips> = HashMap::new();

            let profit_eth = swap.abs_profit_eth();

            if let Some(gas_cost) = gas_cost {
                if profit_eth < gas_cost {
                    error!(
                        profit_eth = NWETH::to_float(profit_eth),
                        gas_cost = NWETH::to_float(gas_cost),
                        %swap,
                        "Profit doesn't exceed the gas cost"
                    );
                    return Err(eyre!("NO_PROFIT_EXCEEDING_GAS"));
                }
            }

            let gas_cost_per_record = gas_cost.unwrap_or_default() / U256::from(swap_vec.len());

            for swap_record in swap_vec.iter() {
                let token_in = swap_record.get_first_token().ok_or_eyre("NO_FIRST_TOKEN")?.clone();

                let profit = swap_record.abs_profit();
                if profit.is_zero() {
                    error!(profit = NWETH::to_float(profit), %swap, "Zero profit");
                    return Err(eyre!("NO_PROFIT"));
                }

                let profit_eth = token_in.calc_eth_value(profit).ok_or_eyre("CALC_ETH_VALUE_FAILED")?;

                let tips = profit_eth.checked_sub(gas_cost_per_record).ok_or_eyre("SUBTRACTION_OVERFLOWN")? * U256::from(tips_pct)
                    / U256::from(10000);
                let min_change = token_in.calc_token_value_from_eth(tips + gas_cost_per_record).unwrap();

                let entry = tips_hashset.entry(token_in.get_address()).or_insert(Tips {
                    token_in,
                    profit: U256::ZERO,
                    profit_eth: U256::ZERO,
                    tips: U256::ZERO,
                    min_change: U256::ZERO,
                });

                entry.profit += profit;
                entry.profit_eth += profit_eth;
                entry.tips += tips;
                entry.min_change += min_change;
            }

            let mut value = U256::ZERO;

            if tips_hashset.iter().any(|(_, x)| x.token_in.is_weth()) {
                let total_tips_eth: U256 = tips_hashset.iter().filter(|(_, x)| x.token_in.is_weth()).map(|(_, x)| x.tips).sum();
                let total_tips_non_eth: U256 = tips_hashset.iter().filter(|(_, x)| !x.token_in.is_weth()).map(|(_, x)| x.tips).sum();
                let total_profit_eth: U256 = tips_hashset.iter().filter(|(_, x)| x.token_in.is_weth()).map(|(_, x)| x.profit_eth).sum();
                for (_, token_tips) in tips_hashset.iter_mut() {
                    if token_tips.token_in.is_weth() {
                        token_tips.tips = total_tips_eth + total_tips_non_eth;
                        if total_tips_eth + total_tips_non_eth > total_profit_eth {
                            value = total_tips_eth + total_tips_non_eth - total_profit_eth;
                        }

                        if value > eth_balance {
                            token_tips.tips = token_tips.tips.checked_sub(value).ok_or_eyre("SUBTRACTION_OVERFLOWN")?;
                            value = eth_balance * U256::from(9000) / U256::from(10000);
                            token_tips.tips += value;
                        }
                    } else {
                        token_tips.tips = U256::ZERO;
                    }
                }
            } else {
                let total_tips = tips_hashset.values().map(|x| x.tips).sum::<U256>();
                value = if total_tips >= eth_balance { eth_balance * U256::from(9000) / U256::from(10000) } else { total_tips };

                for (idx, (_, token_tips)) in tips_hashset.iter_mut().enumerate() {
                    token_tips.tips = if idx == 0 { value } else { U256::ZERO };
                }
            }
            let tips_vec = tips_hashset.values().cloned().collect();

            Ok((tips_vec, value + U256::from(100)))
        }

        _ => Err(eyre!("NOT_IMPLEMENTED")),
    }
}
