use alloy_primitives::Address;

pub trait Protocol {
    fn get_pool_address_vec_for_tokens(token0 : Address, token1 : Address) -> Vec<Address>;
}