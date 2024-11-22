use loom_core_actors::Broadcaster;
use loom_evm_db::DatabaseLoomExt;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::BlockHistoryState;
use loom_types_events::{MessageBackrunTxCompose, StateUpdateEvent};
use revm::{Database, DatabaseCommit, DatabaseRef};

#[derive(Clone)]
pub struct Strategy<DB: Clone + Send + Sync + 'static, LDT: LoomDataTypes + 'static = LoomDataTypesEthereum> {
    compose_channel: Broadcaster<MessageBackrunTxCompose<DB, LDT>>,
    state_update_channel: Broadcaster<StateUpdateEvent<DB, LDT>>,
}

impl<DB: DatabaseRef + Database + DatabaseCommit + BlockHistoryState + DatabaseLoomExt + Send + Sync + Clone + Default + 'static>
    Strategy<DB, LoomDataTypesEthereum>
{
    pub fn new() -> Self {
        let compose_channel: Broadcaster<MessageBackrunTxCompose<DB, LoomDataTypesEthereum>> = Broadcaster::new(100);
        let state_update_channel: Broadcaster<StateUpdateEvent<DB, LoomDataTypesEthereum>> = Broadcaster::new(100);
        Strategy { compose_channel, state_update_channel }
    }
}

impl<DB: Send + Sync + Clone + 'static> Strategy<DB, LoomDataTypesEthereum> {
    pub fn compose_channel(&self) -> Broadcaster<MessageBackrunTxCompose<DB, LoomDataTypesEthereum>> {
        self.compose_channel.clone()
    }

    pub fn state_update_channel(&self) -> Broadcaster<StateUpdateEvent<DB, LoomDataTypesEthereum>> {
        self.state_update_channel.clone()
    }
}
