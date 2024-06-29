use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};

use alloy::{
    primitives::{B128, B256, BlockHash, BlockNumber, U128},
    rpc::{
        json_rpc::{
            Id, Request, RequestPacket, Response, ResponsePacket, ResponsePayload, SerializedRequest,
        },
        types::{
            Block,
            BlockNumberOrTag, trace::geth::GethDebugTracingOptions, TransactionRequest,
        },
    },
    transports::{
        http::{Http, ReqwestTransport},
        TransportError, TransportErrorKind, TransportFut,
    },
};
use eyre::{eyre, OptionExt, Result};
use log::{debug, trace};
use rand::Rng;
use reqwest::Client;
use serde_json::value::RawValue;
use tokio::sync::RwLock;
use tower::Service;
use url::Url;

use crate::cachefolder::CacheFolder;

#[derive(Clone)]
pub struct HttpCachedTransport {
    client: Http<Client>,
    block_number: Arc<AtomicU64>,
    block_filters: Arc<RwLock<HashMap<U128, BlockNumber>>>,
    block_hashes: Arc<RwLock<HashMap<BlockNumber, B256>>>,
    cache_folder: Option<CacheFolder>,
}


impl HttpCachedTransport {
    pub async fn new(url: Url, cache_path: Option<&str>) -> Self {
        let client = ReqwestTransport::new(url);
        let cache_folder = match cache_path {
            Some(path) => {
                Some(CacheFolder::new(path).await)
            }
            None => None
        };
        Self {
            client,
            block_number: Arc::new(AtomicU64::new(0)),
            block_filters: Arc::new(RwLock::new(HashMap::new())),
            block_hashes: Arc::new(RwLock::new(HashMap::new())),
            cache_folder,
        }
    }


    pub fn set_block_number(&self, block_number: u64) -> u64 {
        self.block_number.swap(block_number, Ordering::Relaxed)
    }

    fn convert_block_number(&self, number_or_tag: BlockNumberOrTag) -> Result<BlockNumberOrTag> {
        let current_block = self.read_block_number();
        match number_or_tag {
            BlockNumberOrTag::Number(x) => {
                if x > current_block {
                    Err(eyre!("INCORRECT_BLOCK_NUMBER"))
                } else {
                    Ok(BlockNumberOrTag::Number(x))
                }
            }
            BlockNumberOrTag::Earliest => {
                Ok(BlockNumberOrTag::Earliest)
            }
            _ => Ok(BlockNumberOrTag::Number(current_block))
        }
    }

    pub async fn read_cached(&self, method: String, params_hash: B256) -> Result<String> {
        match &self.cache_folder {
            Some(cf) => {
                cf.read(method, params_hash).await
            }
            None => {
                Err(eyre!("NO_CACHE"))
            }
        }
    }


    pub async fn write_cached(&self, method: String, params_hash: B256, data: String) -> Result<()> {
        match &self.cache_folder {
            Some(cf) => {
                cf.write(method, params_hash, data).await
            }
            None => {
                Err(eyre!("NO_CACHE"))
            }
        }
    }

    pub fn next_block_number(&self) -> u64 {
        let next_block_number = self.block_number.fetch_add(1, Ordering::Relaxed);
        next_block_number
    }

    pub async fn fetch_next_block(&self) -> Result<u64, TransportError> {
        let next_block_number = self.read_block_number() + 1;

        let new_req = Request::<(BlockNumberOrTag, bool)>::new(
            "eth_getBlockByNumber", Id::None, (BlockNumberOrTag::Number(next_block_number), false),
        );

        let new_req: SerializedRequest = new_req.try_into().map_err(|e| TransportError::SerError(e))?;

        match self.cached_or_execute(new_req).await {
            Ok(new_block_packet) => {
                debug!("fetch_next_block : {:?}", new_block_packet);
                if let ResponsePacket::Single(new_block_response) = new_block_packet {
                    let response: Block = serde_json::from_str(new_block_response.payload.as_success().unwrap().get()).map_err(|e| TransportError::DeserError { err: e, text: "err".to_string() })?;
                    self.block_hashes.write().await.insert(next_block_number, response.header.hash.unwrap_or_default());
                    self.set_block_number(next_block_number);
                }
            }
            Err(e) => {}
        }


        Ok(next_block_number)
    }

    pub fn read_block_number(&self) -> u64 {
        self.block_number.load(Ordering::Relaxed)
    }

    pub async fn create_block_filter(&self) -> U128 {
        let filter_id = B128::random();
        let filter_id = U128::try_from(filter_id).unwrap_or_default();
        self.block_filters.write().await.insert(filter_id, self.read_block_number());
        filter_id
    }

    pub async fn get_block_number(self) -> Result<ResponsePacket, TransportError> {
        let block_number = self.read_block_number();
        let value = RawValue::from_string(format!("{}", block_number).to_string()).unwrap();
        let body = Response { id: Id::None, payload: ResponsePayload::Success(value) };
        Ok(ResponsePacket::Single(body))
    }
    pub async fn new_block_filter(self) -> Result<ResponsePacket, TransportError> {
        let filter_id = self.create_block_filter().await;
        let value = format!("\"0x{:x}\"", filter_id).to_string();
        let value = RawValue::from_string(value).unwrap();
        let body = Response { id: Id::None, payload: ResponsePayload::Success(value) };
        Ok(ResponsePacket::Single(body))
    }

    pub async fn get_filter_changes(self, req: SerializedRequest) -> Result<ResponsePacket, TransportError> {
        let raw_value: Vec<U128> = serde_json::from_str(req.params().unwrap().get()).map_err(|e| TransportError::DeserError { err: e, text: "err".to_string() })?;
        trace!("get_filter_changes req : {:?}", raw_value);
        let mut block_filters_guard = self.block_filters.write().await;
        let block_hashes_guard = self.block_hashes.read().await;
        let current_block = self.read_block_number();
        let mut missed_blocks: Vec<BlockHash> = Vec::new();

        for filter_id in raw_value {
            if let Some(filter_block) = block_filters_guard.get(&filter_id).cloned() {
                if filter_block < current_block {
                    block_filters_guard.insert(filter_id, current_block);
                    missed_blocks = (filter_block + 1..=current_block).into_iter().map(|block_number| block_hashes_guard.get(&block_number).cloned().unwrap_or_default()).collect();
                    break;
                }
            }
        }
        let resp_string = serde_json::to_string(&missed_blocks).map_err(TransportError::SerError)?;

        let new_resp = RawValue::from_string(resp_string).map_err(|e| TransportError::DeserError { err: e, text: "err".to_string() })?;

        trace!("get_filter_changes resp : {:?}", new_resp);

        let body = Response { id: Id::None, payload: ResponsePayload::Success(new_resp) };
        Ok(ResponsePacket::Single(body))
    }


    pub async fn cached_or_execute(&self, req: SerializedRequest) -> Result<ResponsePacket, TransportError> {
        let req_hash = req.params_hash();
        let method = req.method().to_string();
        let resp = match self.read_cached(method.clone(), req_hash).await {
            Ok(cached) => {
                let value = RawValue::from_string(cached).unwrap();
                let body = Response { id: Id::None, payload: ResponsePayload::Success(value) };
                Ok(ResponsePacket::Single(body))
            }
            Err(_) => {
                let mut client = self.client.clone();
                match client.call(RequestPacket::Single(req)).await {
                    Ok(resp) => {
                        if let ResponsePacket::Single(resp) = resp.clone() {
                            self.write_cached(method, req_hash, resp.payload.as_success().unwrap().to_string()).await;
                        }
                        Ok(resp)
                    }
                    Err(e) => {
                        Err(e)
                    }
                }
            }
        };
        resp
    }

    pub async fn eth_call(mut self, req: SerializedRequest) -> Result<ResponsePacket, TransportError> {
        let request: (TransactionRequest, BlockNumberOrTag) = serde_json::from_str(req.params().unwrap().get()).map_err(|e| TransportError::DeserError { err: e, text: "err".to_string() })?;
        debug!("call req : {:?}", request);

        let new_req = Request::<(TransactionRequest, BlockNumberOrTag)>::new(
            "eth_call", req.id().clone(), (request.0, self.convert_block_number(request.1).map_err(|e| TransportErrorKind::custom_str("BAD_BLOCK"))?),
        );
        let new_req: SerializedRequest = new_req.try_into().unwrap();

        let resp = self.cached_or_execute(new_req.clone()).await;
        trace!("call resp : {:?}", resp);
        resp
    }


    pub async fn eth_get_block_by_number(mut self, req: SerializedRequest) -> Result<ResponsePacket, TransportError> {
        let request: (BlockNumberOrTag, bool) = serde_json::from_str(req.params().unwrap().get()).map_err(|e| TransportError::DeserError { err: e, text: "err".to_string() })?;
        debug!("get_block_by_number : {:?}", request);

        let new_req = Request::<(BlockNumberOrTag, bool)>::new(
            "eth_getBlockByNumber", req.id().clone(), (self.convert_block_number(request.0).map_err(|e| TransportErrorKind::custom_str("BAD_BLOCK"))?, request.1),
        );

        let new_req: SerializedRequest = new_req.try_into().unwrap();

        let resp = self.cached_or_execute(new_req.clone()).await;
        //println!("get_block_by_number resp : {:?}", resp);
        resp
    }

    pub async fn eth_get_block_by_hash(mut self, req: SerializedRequest) -> Result<ResponsePacket, TransportError> {
        debug!("get_block_by_hash req : {:?}", req);
        let resp = self.cached_or_execute(req.clone()).await;
        resp
    }

    pub async fn debug_trace_block_by_number(mut self, req: SerializedRequest) -> Result<ResponsePacket, TransportError> {
        let request: (BlockNumberOrTag, GethDebugTracingOptions) = serde_json::from_str(req.params().unwrap().get()).map_err(|e| TransportError::DeserError { err: e, text: "err".to_string() })?;
        debug!("debug_trace_block_by_number : {:?}", request);

        let new_req = Request::<(BlockNumberOrTag, GethDebugTracingOptions)>::new(
            "debug_traceBlockByNumber", req.id().clone(), (self.convert_block_number(request.0).map_err(|e| TransportErrorKind::custom_str("BAD_BLOCK"))?, request.1),
        );

        let new_req: SerializedRequest = new_req.try_into().unwrap();

        let resp = self.cached_or_execute(new_req.clone()).await;
        trace!("debug_trace_block_by_number resp : {:?}", resp);
        resp
    }

    pub async fn debug_trace_block_by_hash(mut self, req: SerializedRequest) -> Result<ResponsePacket, TransportError> {
        println!("debug_trace_block_by_hash req : {:?}", req);
        let resp = self.cached_or_execute(req.clone()).await;
        resp
    }


    pub async fn eth_get_logs(mut self, req: SerializedRequest) -> Result<ResponsePacket, TransportError> {
        //TODO: block number check
        debug!("eth_get_logs req  : {:?}", req);
        let resp = self.cached_or_execute(req.clone()).await;
        trace!("eth_get_logs resp : {:?}", resp);
        resp
    }
}


impl Service<RequestPacket> for HttpCachedTransport {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx)
    }

    fn call(&mut self, req: RequestPacket) -> Self::Future {
        match req {
            RequestPacket::Single(single_req) => {
                trace!("Singlereq id : {} method : {} meta : {:?} params :{:?}", single_req.id(), single_req.method(), single_req.meta(), single_req.params());

                let mut self_clone = self.clone();
                match single_req.method() {
                    "eth_blockNumber" | "get_block_number" => {
                        Box::pin(self_clone.get_block_number())
                    }
                    "eth_newBlockFilter" => {
                        Box::pin(self_clone.new_block_filter())
                    }
                    "eth_getFilterChanges" => {
                        Box::pin(self_clone.get_filter_changes(single_req))
                    }
                    "eth_call" => {
                        Box::pin(self_clone.eth_call(single_req))
                    }
                    "eth_getLogs" => {
                        Box::pin(self_clone.eth_get_logs(single_req))
                    }
                    "eth_getBlockByNumber" => {
                        Box::pin(self_clone.eth_get_block_by_number(single_req))
                    }
                    "eth_getBlockByHash" => {
                        Box::pin(self_clone.eth_get_block_by_hash(single_req))
                    }
                    "debug_traceBlockByHash" => {
                        Box::pin(self_clone.debug_trace_block_by_hash(single_req))
                    }
                    "debug_traceBlockByNumber" => {
                        Box::pin(self_clone.debug_trace_block_by_number(single_req))
                    }
                    _ => {
                        Box::pin(async move {
                            match self_clone.client.call(RequestPacket::Single(single_req)).await {
                                Ok(response) => {
                                    match &response {
                                        ResponsePacket::Single(single_resp) => {
                                            trace!("responsepacket response : {:?} ", single_resp);
                                            trace!("responsepacket payload id : {} len {}", single_resp.id, single_resp.payload.as_success().unwrap().get().len());
                                        }
                                        ResponsePacket::Batch(batch_resp) => {}
                                    }
                                    Ok(response)
                                }
                                Err(e) => {
                                    Err(e)
                                }
                            }
                        })
                    }
                }




                /*Box::pin(async move {
                    if single_req.method() == "eth_blockNumber" || single_req.method() == "get_block_number" {
                        let value = RawValue::from_string(format!("{}", block_number).to_string()).unwrap();
                        let body = Response { id: Id::None, payload: ResponsePayload::Success(value) };
                        let packet = ResponsePacket::Single(body);
                        Ok(packet)
                    } else if single_req.method() == "eth_newBlockFilter" {
                        let filter_id = self_clone.new_block_filter().await;

                        let value = RawValue::from_string(format!("{}", filter_id).to_string()).unwrap();
                        let body = Response { id: Id::None, payload: ResponsePayload::Success(value) };
                        let packet = ResponsePacket::Single(body);
                        Ok(packet)
                    } else {
                        match client_clone.call(RequestPacket::Single(single_req)).await {
                            Ok(response) => {
                                match &response {
                                    ResponsePacket::Single(single_resp) => {
                                        //println!("responsepacket payload id : {} len {}", single_resp.id, single_resp.payload.as_success().unwrap().get().len())
                                    }
                                    ResponsePacket::Batch(batch_resp) => {}
                                }
                                Ok(response)
                            }
                            Err(e) => {
                                Err(e)
                            }
                        }
                    }
                })

                 */
            }
            _ => {
                self.client.call(req)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_provider::ext::DebugApi;
    use alloy_rpc_client::{ClientBuilder, RpcClient};
    use eyre::Result;
    use futures::StreamExt;
    use tokio::select;
    use url::Url;

    use alloy_rpc_types::{BlockNumberOrTag, BlockTransactionsKind, Filter};
    use alloy_rpc_types_trace::geth::{GethDebugBuiltInTracerType, GethDebugTracerType, GethDebugTracingOptions, PreStateConfig};

    use crate::httpcached::HttpCachedTransport;

    #[tokio::test]
    async fn test_create_service() -> Result<()> {
        let transport = HttpCachedTransport::new("http://falcon.loop:8008/rpc".parse()?, Some("./.cache")).await;

        let client = RpcClient::new(transport.clone(), true);
        let provider = ProviderBuilder::new().on_client(client);

        let block_number = provider.get_block_number().await?;
        println!("Hello, block {block_number}");
        transport.set_block_number(2000001);
        let block_number = provider.get_block_number().await?;
        println!("Hello, block {block_number}");


        /*let block = provider.get_block_by_number(BlockNumberOrTag::Number(block_number), false).await?.unwrap();
        let trace_opts = GethDebugTracingOptions::default().with_tracer(GethDebugTracerType::BuiltInTracer(GethDebugBuiltInTracerType::PreStateTracer));

        let _ = provider.debug_trace_block_by_number(BlockNumberOrTag::Number(block_number), trace_opts.clone()).await?;
        let _ = provider.debug_trace_block_by_hash(block.header.hash.unwrap(), trace_opts.clone()).await?;

         */

        Ok(())
    }


    #[tokio::test]
    async fn test_get_block_number() -> Result<()> {
        env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug,alloy_rpc_client=off,"));
        let transport = HttpCachedTransport::new(Url::parse("http://falcon.loop:8008/rpc")?, Some("./.cache")).await;
        transport.set_block_number(20179184);

        let client = ClientBuilder::default().transport(transport.clone(), true).with_poll_interval(Duration::from_millis(50)).boxed();
        let provider = ProviderBuilder::new().on_client(client);


        //let provider = ArchiveHistoryProvider::new(provider, 20179184, 20179284);

        let block_number = provider.get_block_number().await?;
        println!("block {block_number}");

        let mut blocks_watcher = provider.watch_blocks().await?.into_stream();

        let weth = defi_abi::IWETH::new("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".parse()?, provider.clone());

        tokio::task::spawn(async move {
            loop {
                select! {
                    block = blocks_watcher.next() => {
                        if let Some(block_vec) = block {
                            for block_hash in block_vec {
                                println!("Block : {:?}", block_hash);
                            }
                        }else{
                            println!("else block : {:?}", block);
                            break;
                        }
                    }
                }
            }
        });

        let trace_opts = GethDebugTracingOptions::default()
            .with_tracer(GethDebugTracerType::BuiltInTracer(GethDebugBuiltInTracerType::PreStateTracer))
            .with_prestate_config(PreStateConfig { diff_mode: Some(true) });

        for i in 0..1000 {
            println!("Set next block: {}", i);
            tokio::time::sleep(Duration::from_millis(10)).await;
            //provider.next_block();
            let current_block_number = transport.fetch_next_block().await?;

            let total_supply = weth.totalSupply().call().await.unwrap();
            println!("Total supply : {}", total_supply._0);

            let filter: Filter = Filter::new().to_block(current_block_number).from_block(current_block_number);
            let logs = provider.get_logs(&filter).await?;
            let block_by_number = provider.get_block_by_number(BlockNumberOrTag::Latest, false).await?.unwrap();
            let block_by_hash = provider.get_block_by_hash(block_by_number.header.hash.unwrap(), BlockTransactionsKind::Full).await?.unwrap();
            assert_eq!(block_by_hash.header, block_by_number.header);

            let trace_block_by_hash = provider.debug_trace_block_by_hash(block_by_number.header.hash.unwrap(), trace_opts.clone()).await?;
            let trace_block_by_number = provider.debug_trace_block_by_number(BlockNumberOrTag::Latest, trace_opts.clone()).await?;
            assert_eq!(trace_block_by_hash, trace_block_by_number);
        }

        Ok(())
    }
}