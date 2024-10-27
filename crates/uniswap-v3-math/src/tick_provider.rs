use alloy::primitives::U256;

pub trait TickProvider {
    fn get_tick(&self, tick: i16) -> eyre::Result<U256>;
}
