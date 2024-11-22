use loom_types_blockchain::SwapError;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};

use crate::Message;

#[derive(Clone, Debug)]
pub enum HealthEvent<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    PoolSwapError(SwapError<LDT>),
    MonitorTx(LDT::TxHash),
}

pub type MessageHealthEvent<LDT = LoomDataTypesEthereum> = Message<HealthEvent<LDT>>;
