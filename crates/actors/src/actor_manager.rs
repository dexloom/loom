use eyre::Result;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::{Actor, WorkerResult};

#[derive(Default)]
pub struct ActorsManager {
    tasks: Vec<JoinHandle<WorkerResult>>,
}

impl ActorsManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, actor: impl Actor + 'static) -> Result<()> {
        match actor.start() {
            Ok(workers) => {
                info!("{} started successfully", actor.name());
                self.tasks.extend(workers);
                Ok(())
            }
            Err(e) => {
                error!("Error starting {} : {}", actor.name(), e);
                Err(e)
            }
        }
    }

    pub fn start_and_wait(&mut self, actor: impl Actor + Send + Sync + 'static) -> Result<()> {
        match actor.start_and_wait() {
            Ok(_) => {
                info!("{} started successfully", actor.name());
                Ok(())
            }
            Err(e) => {
                error!("Error starting {} : {}", actor.name(), e);
                Err(e)
            }
        }
    }

    pub async fn wait(self) {
        let mut f_remaining_futures = self.tasks;
        let mut futures_counter = f_remaining_futures.len();

        while futures_counter > 0 {
            let (result, _index, remaining_futures) = futures::future::select_all(f_remaining_futures).await;
            match result {
                Ok(work_result) => match work_result {
                    Ok(s) => {
                        info!("ActorWorker {_index} finished : {s}")
                    }
                    Err(e) => {
                        error!("ActorWorker {_index} finished with error : {e}")
                    }
                },
                Err(e) => {
                    error!("ActorWorker join error {_index} : {e}")
                }
            }
            f_remaining_futures = remaining_futures;
            futures_counter = f_remaining_futures.len();
        }
    }
}
