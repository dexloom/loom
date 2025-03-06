pub use actor::{Accessor, Actor, ActorResult, Consumer, Producer, WorkerResult};
pub use actor_manager::ActorsManager;
pub use channels::{Broadcaster, MultiProducer};
pub use shared_state::SharedState;

mod actor;
mod actor_manager;
mod channels;
mod shared_state;

#[macro_export]
macro_rules! run_async {
    ($fx:expr) => {
        match $fx.await {
            Ok(_) => {}
            Err(e) => error!("ERROR_RUNNING_ASYNC"),
        }
    };
}

#[macro_export]
macro_rules! run_sync {
    ($fx:expr) => {
        match $fx {
            Ok(_) => {}
            Err(error) => tracing::error!(%error, "ERROR_RUNNING_SYNC"),
        }
    };
}

#[inline]
pub fn subscribe_helper<A: Clone + Send + Sync>(broadcaster: &Broadcaster<A>) -> tokio::sync::broadcast::Receiver<A> {
    broadcaster.subscribe()
}

#[macro_export]
macro_rules! subscribe {
    ($name:ident) => {
        let mut $name = $crate::subscribe_helper(&$name);
    };
}
