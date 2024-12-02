use crate::Message;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::SwapError;

#[derive(Clone, Debug)]
pub enum HealthEvent<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    PoolSwapError(SwapError<LDT>),
    MonitorTx(LDT::TxHash),
}

pub type MessageHealthEvent<LDT = LoomDataTypesEthereum> = Message<HealthEvent<LDT>>;
