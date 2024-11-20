use loom_types_blockchain::loom_data_types::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_blockchain::SwapError;

use crate::Message;

#[derive(Clone, Debug)]
pub enum HealthEvent<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    PoolSwapError(SwapError<LDT>),
    MonitorTx(LDT::TxHash),
}

pub type MessageHealthEvent<LDT = LoomDataTypesEthereum> = Message<HealthEvent<LDT>>;
