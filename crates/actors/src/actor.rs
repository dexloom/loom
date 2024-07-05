use async_trait::async_trait;
use eyre::Result;
use tokio::task::JoinHandle;

use crate::channels::Broadcaster;
use crate::shared_state::SharedState;

pub type WorkerResult = Result<String>;

pub type ActorResult = Result<Vec<JoinHandle<WorkerResult>>>;


#[async_trait]
pub trait Actor
{
    async fn start(&self) -> ActorResult;
    fn name(&self) -> &'static str;
}

pub trait Producer<T>
where
    T: Sync + Send + Clone,
{
    fn produce(&mut self, _broadcaster: Broadcaster<T>) -> &mut Self {
        panic!("Not implemented");
    }
}

pub trait Consumer<T>
where
    T: Sync + Send + Clone,
{
    fn consume(&mut self, _receiver: Broadcaster<T>) -> &mut Self {
        panic!("Not implemented");
    }
}

pub trait Accessor<T> {
    fn access(&mut self, _data: SharedState<T>) -> &mut Self {
        panic!("Not implemented");
    }
}


#[cfg(test)]
mod test {
    use crate::actor::{Actor, Consumer, Producer, SharedState};
    use crate::channels::Broadcaster;

    //use crate::macros::*;

    #[derive(Clone)]
    struct DataStruct0 {
        //#[accessor(data, i32)]
        data: Option<SharedState<i32>>,
    }

    #[derive(Clone)]
    struct DataStruct1 {
        data: String,
    }

    #[derive(Clone)]
    struct DataStruct2 {
        pub data: u32,
    }

    #[derive(Clone)]
    struct DataStruct3 {
        data: u128,
    }

    //#[impl_accessor(PendingTxStateChangeProcessorActor, mempool)]
    struct TestActor {
        state: Option<SharedState<DataStruct0>>,
        broadcaster0: Option<Broadcaster<DataStruct0>>,
        broadcaster1: Option<Broadcaster<DataStruct1>>,
        consumer2: Option<Broadcaster<DataStruct2>>,

    }

    impl TestActor {
        pub fn new() -> Self {
            Self {
                state: None,
                broadcaster0: None,
                broadcaster1: None,
                consumer2: None,
            }
        }

        pub async fn start(&self) {
            loop {
                //tokio::select! {
                /*let msg = self.consumer2.recv() => {

                }

                 */

                //}
            }
        }
    }


    impl Consumer<DataStruct2> for TestActor {
        fn consume(&mut self, consumer: Broadcaster<DataStruct2>) -> &mut Self {
            self.consumer2 = Some(consumer);
            self
        }
    }


    impl Producer<DataStruct0> for TestActor {
        fn produce(&mut self, broadcaster: Broadcaster<DataStruct0>) -> &mut Self {
            self.broadcaster0 = Some(broadcaster);
            self
        }
    }

    impl Producer<DataStruct1> for TestActor {
        fn produce(&mut self, broadcaster: Broadcaster<DataStruct1>) -> &mut Self {
            self.broadcaster1 = Some(broadcaster);
            self
        }
    }


    #[tokio::test]
    async fn test_actor() {
        let channel0: Broadcaster<DataStruct0> = Broadcaster::new(10);
        let channel1: Broadcaster<DataStruct1> = Broadcaster::new(10);
        let channel2: Broadcaster<DataStruct2> = Broadcaster::new(10);


        let mut test_actor: TestActor = TestActor::new();
        test_actor
            .produce(channel0)
            .produce(channel1)
            .consume(channel2)
            .start().await;
    }
}
