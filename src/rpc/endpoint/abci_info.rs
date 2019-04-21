//! `/abci_info` endpoint JSONRPC wrapper

use crate::{block, hash, rpc, Hash};
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};
use subtle_encoding::base64;

/// Request ABCI information from a node
#[derive(Debug, Default)]
pub struct Request;

impl rpc::Request for Request {
    type Response = Response;

    fn path(&self) -> rpc::request::Path {
        "/abci_info".parse().unwrap()
    }
}

/// ABCI information response
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Response {
    /// ABCI info
    pub response: AbciInfoResponse,
}

impl rpc::Response for Response {}

/// ABCI information
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AbciInfoResponse {
    /// Name of the application
    pub data: String,

    /// Version
    pub version: Option<String>,

    /// Last block height
    pub last_block_height: block::Height,

    /// Last app hash for the block
    #[serde(
        serialize_with = "serialize_app_hash",
        deserialize_with = "parse_app_hash"
    )]
    pub last_block_app_hash: Hash,
}

/// Parse Base64-encoded app hash
#[cfg(feature = "rpc")]
pub(crate) fn parse_app_hash<'de, D>(deserializer: D) -> Result<Hash, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes = base64::decode(String::deserialize(deserializer)?.as_bytes())
        .map_err(|e| D::Error::custom(format!("{}", e)))?;

    Hash::new(hash::Algorithm::Sha256, &bytes).map_err(|e| D::Error::custom(format!("{}", e)))
}

/// Serialize Base64-encoded app hash
#[cfg(feature = "rpc")]
pub(crate) fn serialize_app_hash<S>(hash: &Hash, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    hash.as_bytes()
        .map(|bytes| String::from_utf8(base64::encode(bytes)).unwrap())
        .unwrap_or_default()
        .serialize(serializer)
}
