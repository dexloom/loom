use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::{EntityAddress, PoolWrapper, SwapLine, SwapStep, Token};
use alloy_primitives::U256;

#[derive(Clone, Debug)]
pub enum Swap {
    None,
    ExchangeSwapLine(SwapLine),
    BackrunSwapSteps((SwapStep, SwapStep)),
    BackrunSwapLine(SwapLine),
    Multiple(Vec<Swap>),
}

impl Display for Swap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Swap::ExchangeSwapLine(path) => write!(f, "{path}"),
            Swap::BackrunSwapLine(path) => write!(f, "{path}"),
            Swap::BackrunSwapSteps((sp0, sp1)) => write!(f, "{sp0} {sp1}"),
            Swap::Multiple(_) => write!(f, "MULTIPLE_SWAP"),
            Swap::None => write!(f, "UNKNOWN_SWAP_TYPE"),
        }
    }
}

impl Swap {
    pub fn to_swap_steps(self: &Swap, multicaller: EntityAddress) -> Option<(SwapStep, SwapStep)> {
        match self {
            Swap::BackrunSwapLine(swap_line) => swap_line.to_swap_steps(multicaller),
            Swap::BackrunSwapSteps((sp0, sp1)) => Some((sp0.clone(), sp1.clone())),
            _ => None,
        }
    }

    pub fn arb_profit(&self) -> U256 {
        match self {
            Swap::BackrunSwapLine(path) => path.abs_profit(),
            Swap::BackrunSwapSteps((sp0, sp1)) => SwapStep::arb_profit(sp0, sp1),
            Swap::Multiple(swap_vec) => swap_vec.iter().map(|x| x.arb_profit()).sum(),
            Swap::None => U256::ZERO,
            Swap::ExchangeSwapLine(_) => U256::ZERO,
        }
    }

    pub fn pre_estimate_gas(&self) -> u64 {
        match self {
            Swap::ExchangeSwapLine(path) => path.gas_used.unwrap_or_default(),
            Swap::BackrunSwapLine(path) => path.gas_used.unwrap_or_default(),
            Swap::BackrunSwapSteps((sp0, sp1)) => {
                sp0.swap_line_vec().iter().map(|i| i.gas_used.unwrap_or_default()).sum::<u64>()
                    + sp1.swap_line_vec().iter().map(|i| i.gas_used.unwrap_or_default()).sum::<u64>()
            }
            Swap::Multiple(swap_vec) => swap_vec.iter().map(|x| x.pre_estimate_gas()).sum(),
            Swap::None => 0,
        }
    }

    pub fn arb_profit_eth(&self) -> U256 {
        match self {
            Swap::ExchangeSwapLine(_) => U256::ZERO,
            Swap::BackrunSwapLine(path) => path.abs_profit_eth(),
            Swap::BackrunSwapSteps((sp0, sp1)) => SwapStep::arb_profit_eth(sp0, sp1),
            Swap::Multiple(swap_vec) => swap_vec.iter().map(|x| x.arb_profit_eth()).sum(),
            Swap::None => U256::ZERO,
        }
    }

    pub fn get_first_token(&self) -> Option<&Arc<Token>> {
        match self {
            Swap::ExchangeSwapLine(swap_path) => swap_path.get_first_token(),
            Swap::BackrunSwapLine(swap_path) => swap_path.get_first_token(),
            Swap::BackrunSwapSteps((sp0, _sp1)) => sp0.get_first_token(),
            Swap::Multiple(_) => None,
            Swap::None => None,
        }
    }

    pub fn get_pool_id_vec(&self) -> Vec<EntityAddress> {
        match self {
            Swap::ExchangeSwapLine(swap_line) => swap_line.pools().iter().map(|item| item.get_pool_id()).collect(),
            Swap::BackrunSwapLine(swap_line) => swap_line.pools().iter().map(|item| item.get_pool_id()).collect(),
            Swap::BackrunSwapSteps((sp0, sp1)) => {
                let mut swap_line_vec = sp0.swap_line_vec().clone();
                swap_line_vec.extend(sp1.swap_line_vec().clone());
                swap_line_vec.iter().flat_map(|item| item.pools().iter().map(|p| p.get_pool_id()).collect::<Vec<_>>()).collect()
            }
            Swap::Multiple(swap_vec) => swap_vec.iter().flat_map(|x| x.get_pool_id_vec()).collect(),
            Swap::None => Vec::new(),
        }
    }

    pub fn get_pool_address_vec(&self) -> Vec<EntityAddress> {
        match self {
            Swap::ExchangeSwapLine(swap_line) => swap_line.pools().iter().map(|item| item.get_address()).collect(),
            Swap::BackrunSwapLine(swap_line) => swap_line.pools().iter().map(|item| item.get_address()).collect(),
            Swap::BackrunSwapSteps((sp0, sp1)) => {
                let mut swap_line_vec = sp0.swap_line_vec().clone();
                swap_line_vec.extend(sp1.swap_line_vec().clone());
                swap_line_vec.iter().flat_map(|item| item.pools().iter().map(|p| p.get_address()).collect::<Vec<_>>()).collect()
            }
            Swap::Multiple(swap_vec) => swap_vec.iter().flat_map(|x| x.get_pool_address_vec()).collect(),
            Swap::None => Vec::new(),
        }
    }

    pub fn get_pools_vec(&self) -> Vec<PoolWrapper> {
        match self {
            Swap::ExchangeSwapLine(swap_line) => swap_line.pools().clone(),
            Swap::BackrunSwapLine(swap_line) => swap_line.pools().clone(),
            Swap::BackrunSwapSteps((sp0, sp1)) => {
                let mut swap_line_vec = sp0.swap_line_vec().clone();
                swap_line_vec.extend(sp1.swap_line_vec().clone());
                swap_line_vec.iter().flat_map(|item| item.pools().iter().cloned()).collect::<Vec<_>>().to_vec()
            }
            Swap::Multiple(swap_vec) => swap_vec.iter().flat_map(|x| x.get_pools_vec()).collect(),
            Swap::None => Vec::new(),
        }
    }
}
