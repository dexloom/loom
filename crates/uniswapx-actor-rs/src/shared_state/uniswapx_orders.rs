use alloy_primitives::FixedBytes;
use std::collections::HashMap;
use uniswapx_client_rs::types::order::Order;

#[derive(Clone, Debug, Default)]
pub struct UniswapXOrders {
    pub open_orders: HashMap<FixedBytes<32>, Order>,
}

impl UniswapXOrders {
    pub fn new() -> UniswapXOrders {
        UniswapXOrders { open_orders: HashMap::new() }
    }
}
