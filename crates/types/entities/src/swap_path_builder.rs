#![allow(clippy::type_complexity)]
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use crate::{Market, PoolWrapper, SwapDirection, SwapPath};
use eyre::Result;
use loom_types_blockchain::LoomDataTypes;

struct SwapPathSet<LDT: LoomDataTypes> {
    set: HashSet<SwapPath<LDT>>,
}

impl<LDT: LoomDataTypes> SwapPathSet<LDT> {
    pub fn new() -> SwapPathSet<LDT> {
        SwapPathSet { set: HashSet::new() }
    }

    pub fn extend(&mut self, path_vec: Vec<SwapPath<LDT>>) {
        for path in path_vec {
            self.set.insert(path);
        }
    }
    pub fn vec(self) -> Vec<SwapPath<LDT>> {
        self.set.into_iter().collect()
    }

    pub fn arc_vec(self) -> Vec<Arc<SwapPath<LDT>>> {
        self.set.into_iter().map(Arc::new).collect()
    }
}

// (Basic -> Token1) -> (Token1 -> Basic)
fn build_swap_path_two_hopes_basic_in<LDT: LoomDataTypes>(
    market: &Market<LDT>,
    pool: &PoolWrapper<LDT>,
    token_from_address: LDT::Address,
    token_to_address: LDT::Address,
) -> Result<Vec<SwapPath<LDT>>> {
    let mut ret: Vec<SwapPath<LDT>> = Vec::new();
    let Some(token_token_pools) = market.get_token_token_pools(&token_to_address, &token_from_address) else {
        return Ok(ret);
    };
    for pool_address in token_token_pools.iter() {
        if market.is_pool_disabled(pool_address) {
            continue;
        }

        let Some(loop_pool) = market.get_pool(pool_address) else { continue };
        let token_from = market.get_token_or_default(&token_from_address);
        let token_to = market.get_token_or_default(&token_to_address);

        let mut swap_path = SwapPath::<LDT>::new_swap(token_from.clone(), token_to.clone(), pool.clone());
        if !swap_path.contains_pool(loop_pool) {
            swap_path.push_swap_hope(token_to, token_from, loop_pool.clone())?;
            ret.push(swap_path)
        }
    }
    Ok(ret)
}
// (Basic -> Token1) -> (Token1 -> Token2) -> (Token2 -> Basic)
fn build_swap_path_three_hopes_basic_in<LDT: LoomDataTypes>(
    market: &Market<LDT>,
    pool: &PoolWrapper<LDT>,
    token_from_address: LDT::Address,
    token_to_address: LDT::Address,
) -> Result<Vec<SwapPath<LDT>>> {
    let mut ret: Vec<SwapPath<LDT>> = Vec::new();
    if market.get_token_pools_len(&token_to_address) < 2 {
        return Ok(ret);
    }
    let Some(token_tokens) = market.get_token_tokens(&token_to_address) else {
        return Ok(ret);
    };

    for token_middle_address in token_tokens.iter() {
        if market.get_token_pools_len(token_middle_address) < 2 {
            continue;
        }

        let Some(token_token_pools_1) = market.get_token_token_pools(&token_to_address, token_middle_address) else { continue };
        let Some(token_token_pools_2) = market.get_token_token_pools(token_middle_address, &token_from_address) else { continue };

        for pool_address_1 in token_token_pools_1.iter() {
            if market.is_pool_disabled(pool_address_1) {
                continue;
            }

            for pool_address_2 in token_token_pools_2.iter() {
                if market.is_pool_disabled(pool_address_2) {
                    continue;
                }

                let Some(pool_1) = market.get_pool(pool_address_1) else { continue };
                let Some(pool_2) = market.get_pool(pool_address_2) else { continue };

                let token_from = market.get_token_or_default(&token_from_address);
                let token_to = market.get_token_or_default(&token_to_address);
                let token_middle = market.get_token_or_default(token_middle_address);

                let mut swap = SwapPath::new_swap(token_from.clone(), token_to.clone(), pool.clone());
                if !swap.contains_pool(pool_1) {
                    let _ = swap.push_swap_hope(token_to, token_middle.clone(), pool_1.clone());
                } else {
                    continue;
                }

                if !swap.contains_pool(pool_2) {
                    let _ = swap.push_swap_hope(token_middle, token_from, pool_2.clone());
                } else {
                    continue;
                }

                ret.push(swap)
            }
        }
    }
    Ok(ret)
}
// (Basic -> Token) -> (Token -> Token) -> (Token -> Token) -> (Token -> Basic)
fn build_swap_path_four_hopes_basic_in<LDT: LoomDataTypes>(
    market: &Market<LDT>,
    pool: &PoolWrapper<LDT>,
    token_from_address: LDT::Address,
    token_to_address: LDT::Address,
) -> Result<Vec<SwapPath<LDT>>> {
    let mut ret: Vec<SwapPath<LDT>> = Vec::new();
    if let Some(token_tokens) = market.get_token_tokens(&token_to_address) {
        for token_middle_address in token_tokens.iter() {
            if !market.get_token_or_default(token_middle_address).is_middle() {
                continue;
            }

            if let Some(token_tokens_0) = market.get_token_tokens(token_middle_address) {
                for token_middle_address_0 in token_tokens_0.iter() {
                    /*if !market.get_token_or_default(token_middle_address_0).is_basic() {
                        continue;
                    }
                     */

                    if let Some(token_token_pools_1) = market.get_token_token_pools(&token_to_address, token_middle_address) {
                        if let Some(token_token_pools_2) = market.get_token_token_pools(token_middle_address, token_middle_address_0) {
                            if let Some(token_token_pools_3) = market.get_token_token_pools(token_middle_address_0, &token_from_address) {
                                for pool_address_1 in token_token_pools_1.iter() {
                                    if market.is_pool_disabled(pool_address_1) {
                                        continue;
                                    }
                                    for pool_address_2 in token_token_pools_2.iter() {
                                        if market.is_pool_disabled(pool_address_2) {
                                            continue;
                                        }
                                        for pool_address_3 in token_token_pools_3.iter() {
                                            if market.is_pool_disabled(pool_address_3) {
                                                continue;
                                            }
                                            if let Some(pool_1) = market.get_pool(pool_address_1) {
                                                if let Some(pool_2) = market.get_pool(pool_address_2) {
                                                    if let Some(pool_3) = market.get_pool(pool_address_3) {
                                                        let token_from = market.get_token_or_default(&token_from_address);
                                                        let token_to = market.get_token_or_default(&token_to_address);
                                                        let token_middle = market.get_token_or_default(token_middle_address);
                                                        let token_middle_0 = market.get_token_or_default(token_middle_address_0);

                                                        let mut swap =
                                                            SwapPath::new_swap(token_from.clone(), token_to.clone(), pool.clone());
                                                        if !swap.contains_pool(pool_1) {
                                                            let _ = swap.push_swap_hope(token_to, token_middle.clone(), pool_1.clone());
                                                        } else {
                                                            continue;
                                                        }

                                                        if !swap.contains_pool(pool_2) {
                                                            let _ =
                                                                swap.push_swap_hope(token_middle, token_middle_0.clone(), pool_2.clone());
                                                        } else {
                                                            continue;
                                                        }

                                                        if !swap.contains_pool(pool_3) {
                                                            let _ = swap.push_swap_hope(token_middle_0, token_from, pool_3.clone());
                                                        } else {
                                                            continue;
                                                        }

                                                        ret.push(swap)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(ret)
}

fn build_swap_path_two_hopes_basic_out<LDT: LoomDataTypes>(
    market: &Market<LDT>,
    pool: &PoolWrapper<LDT>,
    token_from_address: LDT::Address,
    token_to_address: LDT::Address,
) -> Result<Vec<SwapPath<LDT>>> {
    let mut ret: Vec<SwapPath<LDT>> = Vec::new();

    if market.get_token_pools_len(&token_from_address) < 2 {
        return Ok(ret);
    }

    if let Some(token_token_pools) = market.get_token_token_pools(&token_to_address, &token_from_address) {
        for pool_address in token_token_pools.iter() {
            if market.is_pool_disabled(pool_address) {
                continue;
            }
            if let Some(loop_pool) = market.get_pool(pool_address) {
                let token_from = market.get_token_or_default(&token_from_address);
                let token_to = market.get_token_or_default(&token_to_address);

                let mut swap = SwapPath::new_swap(token_from.clone(), token_to.clone(), pool.clone());
                if !swap.contains_pool(loop_pool) {
                    let _ = swap.insert_swap_hope(token_to, token_from, loop_pool.clone());
                    ret.push(swap)
                }
            }
        }
    }
    Ok(ret)
}

fn build_swap_path_three_hopes_basic_out<LDT: LoomDataTypes>(
    market: &Market<LDT>,
    pool: &PoolWrapper<LDT>,
    token_from_address: LDT::Address,
    token_to_address: LDT::Address,
) -> Result<Vec<SwapPath<LDT>>> {
    let mut ret: Vec<SwapPath<LDT>> = Vec::new();
    let Some(token_tokens) = market.get_token_tokens(&token_from_address) else {
        return Ok(vec![]);
    };
    if market.get_token_pools_len(&token_from_address) < 2 {
        return Ok(ret);
    }

    for token_middle_address in token_tokens.iter() {
        if market.get_token_pools_len(token_middle_address) < 2 {
            continue;
        }

        let Some(token_token_pools_1) = market.get_token_token_pools(&token_to_address, token_middle_address) else { continue };
        let Some(token_token_pools_2) = market.get_token_token_pools(token_middle_address, &token_from_address) else { continue };
        for pool_address_1 in token_token_pools_1.iter() {
            if market.is_pool_disabled(pool_address_1) {
                continue;
            }

            for pool_address_2 in token_token_pools_2.iter() {
                if market.is_pool_disabled(pool_address_2) {
                    continue;
                }
                let Some(pool_1) = market.get_pool(pool_address_1) else { continue };
                let Some(pool_2) = market.get_pool(pool_address_2) else { continue };
                let token_from = market.get_token_or_default(&token_from_address);
                let token_to = market.get_token_or_default(&token_to_address);
                let token_middle = market.get_token_or_default(token_middle_address);

                let mut swap = SwapPath::new_swap(token_from.clone(), token_to.clone(), pool.clone());
                if !swap.contains_pool(pool_2) {
                    let _ = swap.insert_swap_hope(token_middle.clone(), token_from.clone(), pool_2.clone());
                } else {
                    continue;
                }

                if !swap.contains_pool(pool_1) {
                    let _ = swap.insert_swap_hope(token_to, token_middle, pool_1.clone());
                } else {
                    continue;
                }

                ret.push(swap)
            }
        }
    }
    Ok(ret)
}

fn build_swap_path_four_hopes_basic_out<LDT: LoomDataTypes>(
    market: &Market<LDT>,
    pool: &PoolWrapper<LDT>,
    token_from_address: LDT::Address,
    token_to_address: LDT::Address,
) -> Result<Vec<SwapPath<LDT>>> {
    let mut ret: Vec<SwapPath<LDT>> = Vec::new();
    if let Some(token_tokens) = market.get_token_tokens(&token_from_address) {
        for token_middle_address in token_tokens.iter() {
            if !market.get_token_or_default(token_middle_address).is_middle() {
                continue;
            }
            if let Some(token_tokens_2) = market.get_token_tokens(token_middle_address) {
                for token_middle_address_0 in token_tokens_2.iter() {
                    /*if !market.get_token_or_default(token_middle_address_0).is_basic() {
                        continue;
                    }
                     */

                    if let Some(token_token_pools_0) = market.get_token_token_pools(&token_to_address, token_middle_address_0) {
                        if let Some(token_token_pools_1) = market.get_token_token_pools(token_middle_address_0, token_middle_address) {
                            if let Some(token_token_pools_2) = market.get_token_token_pools(token_middle_address, &token_from_address) {
                                for pool_address_0 in token_token_pools_0.iter() {
                                    if market.is_pool_disabled(pool_address_0) {
                                        continue;
                                    }

                                    for pool_address_1 in token_token_pools_1.iter() {
                                        if market.is_pool_disabled(pool_address_1) {
                                            continue;
                                        }

                                        for pool_address_2 in token_token_pools_2.iter() {
                                            if market.is_pool_disabled(pool_address_2) {
                                                continue;
                                            }

                                            if let Some(pool_0) = market.get_pool(pool_address_0) {
                                                if let Some(pool_1) = market.get_pool(pool_address_1) {
                                                    if let Some(pool_2) = market.get_pool(pool_address_2) {
                                                        let token_from = market.get_token_or_default(&token_from_address);
                                                        let token_to = market.get_token_or_default(&token_to_address);
                                                        let token_middle = market.get_token_or_default(token_middle_address);
                                                        let token_middle_0 = market.get_token_or_default(token_middle_address_0);

                                                        let mut swap =
                                                            SwapPath::new_swap(token_from.clone(), token_to.clone(), pool.clone());

                                                        if !swap.contains_pool(pool_2) {
                                                            let _ = swap.insert_swap_hope(
                                                                token_middle.clone(),
                                                                token_from.clone(),
                                                                pool_2.clone(),
                                                            );
                                                        } else {
                                                            continue;
                                                        }

                                                        if !swap.contains_pool(pool_1) {
                                                            let _ =
                                                                swap.insert_swap_hope(token_middle_0.clone(), token_middle, pool_1.clone());
                                                        } else {
                                                            continue;
                                                        }

                                                        if !swap.contains_pool(pool_0) {
                                                            let _ = swap.insert_swap_hope(token_to, token_middle_0, pool_0.clone());
                                                        } else {
                                                            continue;
                                                        }

                                                        ret.push(swap)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(ret)
}

// (Token -> Token) -> (Token -> Token) -> (Token -> Token)
fn build_swap_path_three_hopes_no_basic<LDT: LoomDataTypes>(
    market: &Market<LDT>,
    pool: &PoolWrapper<LDT>,
    token_from_address: LDT::Address,
    token_to_address: LDT::Address,
) -> Result<Vec<SwapPath<LDT>>> {
    let mut ret: Vec<SwapPath<LDT>> = Vec::new();

    if let Some(token_tokens) = market.get_token_tokens(&token_from_address) {
        for token_basic_address in token_tokens.iter() {
            let token_basic = market.get_token_or_default(token_basic_address);
            if !token_basic.is_basic() {
                continue;
            }

            if let Some(token_token_pools_1) = market.get_token_token_pools(token_basic_address, &token_from_address) {
                if let Some(token_token_pools_2) = market.get_token_token_pools(&token_to_address, token_basic_address) {
                    for pool_address_1 in token_token_pools_1.iter() {
                        if market.is_pool_disabled(pool_address_1) {
                            continue;
                        }

                        for pool_address_2 in token_token_pools_2.iter() {
                            if market.is_pool_disabled(pool_address_2) {
                                continue;
                            }

                            if let Some(pool_1) = market.get_pool(pool_address_1) {
                                if let Some(pool_2) = market.get_pool(pool_address_2) {
                                    let token_from = market.get_token_or_default(&token_from_address);
                                    let token_to = market.get_token_or_default(&token_to_address);

                                    let mut swap = SwapPath::new_swap(token_from.clone(), token_to.clone(), pool.clone());
                                    if !swap.contains_pool(pool_1) {
                                        let _ = swap.insert_swap_hope(token_basic.clone(), token_from.clone(), pool_1.clone());
                                    } else {
                                        continue;
                                    }

                                    if !swap.contains_pool(pool_2) {
                                        let _ = swap.push_swap_hope(token_to, token_basic.clone(), pool_2.clone());
                                    } else {
                                        continue;
                                    }

                                    ret.push(swap)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(ret)
}

pub fn build_swap_path_vec<LDT: LoomDataTypes>(
    market: &Market<LDT>,
    directions: &BTreeMap<PoolWrapper<LDT>, Vec<SwapDirection<LDT>>>,
) -> Result<Vec<SwapPath<LDT>>> {
    let mut ret_map = SwapPathSet::new();

    for (pool, directions) in directions.iter() {
        for direction in directions.iter() {
            let token_from_address = *direction.from();
            let token_to_address = *direction.to();

            if market.is_basic_token(&token_to_address) {
                ret_map.extend(build_swap_path_two_hopes_basic_out(market, pool, token_from_address, token_to_address)?);
                ret_map.extend(build_swap_path_three_hopes_basic_out(market, pool, token_from_address, token_to_address)?);
                // TODO : Add this later
                //ret_map.extend(build_swap_path_four_hopes_basic_out(market, pool, token_from_address, token_to_address)?);
            }

            if market.is_basic_token(&token_from_address) {
                ret_map.extend(build_swap_path_two_hopes_basic_in(market, pool, token_from_address, token_to_address)?);
                ret_map.extend(build_swap_path_three_hopes_basic_in(market, pool, token_from_address, token_to_address)?);

                // TODO : Add this later
                /*if market.is_basic_token(&token_to_address) {
                    ret_map.extend(build_swap_path_four_hopes_basic_in(market, pool, token_from_address, token_to_address)?);
                }*/
            }

            if (!market.is_basic_token(&token_from_address) && !market.is_basic_token(&token_to_address))
                || ((token_from_address != LDT::WETH) && (token_to_address != LDT::WETH))
            {
                ret_map.extend(build_swap_path_three_hopes_no_basic(market, pool, token_from_address, token_to_address)?);
            }
        }
    }

    Ok(ret_map.vec())
}
