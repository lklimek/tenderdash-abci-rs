//! Blocking ABCI client.

use crate::codec::ClientCodec;
use crate::{Error, Result};
use std::net::{TcpStream, ToSocketAddrs};
use tendermint_proto::abci::{
    request, response, RequestApplySnapshotChunk, RequestBeginBlock, RequestCheckTx, RequestCommit,
    RequestDeliverTx, RequestEndBlock, RequestFlush, RequestInfo, RequestInitChain,
    RequestListSnapshots, RequestLoadSnapshotChunk, RequestOfferSnapshot, RequestQuery,
    RequestSetOption, ResponseApplySnapshotChunk, ResponseBeginBlock, ResponseCheckTx,
    ResponseCommit, ResponseDeliverTx, ResponseEndBlock, ResponseFlush, ResponseInfo,
    ResponseInitChain, ResponseListSnapshots, ResponseLoadSnapshotChunk, ResponseOfferSnapshot,
    ResponseQuery, ResponseSetOption,
};
use tendermint_proto::abci::{Request, RequestEcho, ResponseEcho};

/// The size of the read buffer for the client in its receiving of responses
/// from the server.
pub const DEFAULT_CLIENT_READ_BUF_SIZE: usize = 1024;

/// Builder for a blocking ABCI client.
pub struct ClientBuilder {
    read_buf_size: usize,
}

impl ClientBuilder {
    /// Builder constructor.
    pub fn new(read_buf_size: usize) -> Self {
        Self { read_buf_size }
    }

    /// Client constructor that attempts to connect to the given network
    /// address.
    pub fn connect<A: ToSocketAddrs>(self, addr: A) -> Result<Client> {
        let stream = TcpStream::connect(addr)?;
        Ok(Client {
            codec: ClientCodec::new(stream, self.read_buf_size),
        })
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            read_buf_size: DEFAULT_CLIENT_READ_BUF_SIZE,
        }
    }
}

/// Blocking ABCI client.
pub struct Client {
    codec: ClientCodec<TcpStream>,
}

macro_rules! perform {
    ($self:expr, $type:ident, $req:expr) => {
        match $self.perform(request::Value::$type($req))? {
            response::Value::$type(r) => Ok(r),
            r => Err(Error::UnexpectedServerResponseType(stringify!($type).to_string(), r).into()),
        }
    };
}

impl Client {
    /// Ask the ABCI server to echo back a message.
    pub fn echo(&mut self, req: RequestEcho) -> Result<ResponseEcho> {
        perform!(self, Echo, req)
    }

    /// Request information about the ABCI application.
    pub fn info(&mut self, req: RequestInfo) -> Result<ResponseInfo> {
        perform!(self, Info, req)
    }

    /// To be called once upon genesis.
    pub fn init_chain(&mut self, req: RequestInitChain) -> Result<ResponseInitChain> {
        perform!(self, InitChain, req)
    }

    /// Query the application for data at the current or past height.
    pub fn query(&mut self, req: RequestQuery) -> Result<ResponseQuery> {
        perform!(self, Query, req)
    }

    /// Check the given transaction before putting it into the local mempool.
    pub fn check_tx(&mut self, req: RequestCheckTx) -> Result<ResponseCheckTx> {
        perform!(self, CheckTx, req)
    }

    /// Signal the beginning of a new block, prior to any `DeliverTx` calls.
    pub fn begin_block(&mut self, req: RequestBeginBlock) -> Result<ResponseBeginBlock> {
        perform!(self, BeginBlock, req)
    }

    /// Apply a transaction to the application's state.
    pub fn deliver_tx(&mut self, req: RequestDeliverTx) -> Result<ResponseDeliverTx> {
        perform!(self, DeliverTx, req)
    }

    /// Signal the end of a block.
    pub fn end_block(&mut self, req: RequestEndBlock) -> Result<ResponseEndBlock> {
        perform!(self, EndBlock, req)
    }

    pub fn flush(&mut self) -> Result<ResponseFlush> {
        perform!(self, Flush, RequestFlush {})
    }

    /// Commit the current state at the current height.
    pub fn commit(&mut self) -> Result<ResponseCommit> {
        perform!(self, Commit, RequestCommit {})
    }

    /// Request that the application set an option to a particular value.
    pub fn set_option(&mut self, req: RequestSetOption) -> Result<ResponseSetOption> {
        perform!(self, SetOption, req)
    }

    /// Used during state sync to discover available snapshots on peers.
    pub fn list_snapshots(&mut self) -> Result<ResponseListSnapshots> {
        perform!(self, ListSnapshots, RequestListSnapshots {})
    }

    /// Called when bootstrapping the node using state sync.
    pub fn offer_snapshot(&mut self, req: RequestOfferSnapshot) -> Result<ResponseOfferSnapshot> {
        perform!(self, OfferSnapshot, req)
    }

    /// Used during state sync to retrieve chunks of snapshots from peers.
    pub fn load_snapshot_chunk(
        &mut self,
        req: RequestLoadSnapshotChunk,
    ) -> Result<ResponseLoadSnapshotChunk> {
        perform!(self, LoadSnapshotChunk, req)
    }

    /// Apply the given snapshot chunk to the application's state.
    pub fn apply_snapshot_chunk(
        &mut self,
        req: RequestApplySnapshotChunk,
    ) -> Result<ResponseApplySnapshotChunk> {
        perform!(self, ApplySnapshotChunk, req)
    }

    fn perform(&mut self, req: request::Value) -> Result<response::Value> {
        self.codec.send(Request { value: Some(req) })?;
        let res = self
            .codec
            .next()
            .ok_or(Error::ServerConnectionTerminated)??;
        match res.value {
            Some(value) => Ok(value),
            None => Err(Error::MalformedServerResponse.into()),
        }
    }
}
