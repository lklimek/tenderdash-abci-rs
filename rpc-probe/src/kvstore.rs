//! Common interactions that can be used by plans interacting with the
//! `kvstore` application running on a Tendermint node.

use crate::plan::{PlannedInteraction, PlannedSubscription};
use crate::request::Request;
use crate::utils::{encode_kvpair, hex_string};
use serde_json::json;

pub fn abci_info() -> PlannedInteraction {
    Request::new("abci_info", json!({})).into()
}

pub fn abci_query(key: &str) -> PlannedInteraction {
    Request::new(
        "abci_query",
        json!({
            "data": hex_string(key),
        }),
    )
    .into()
}

pub fn block(height: u64) -> PlannedInteraction {
    Request::new(
        "block",
        json!({
            "height": format!("{}", height),
        }),
    )
    .into()
}

pub fn block_results(height: u64) -> PlannedInteraction {
    Request::new(
        "block_results",
        json!({
            "height": format!("{}", height),
        }),
    )
    .into()
}

pub fn blockchain(min_height: u64, max_height: u64) -> PlannedInteraction {
    Request::new(
        "blockchain",
        json!({
            "minHeight": format!("{}", min_height),
            "maxHeight": format!("{}", max_height),
        }),
    )
    .into()
}

pub fn broadcast_tx(method: &str, key: &str, value: &str) -> PlannedInteraction {
    Request::new(
        format!("broadcast_tx_{}", method).as_str(),
        json!({
            "tx": encode_kvpair(key, value),
        }),
    )
    .into()
}

pub fn commit(height: u64) -> PlannedInteraction {
    Request::new(
        "commit",
        json!({
            "height": format!("{}", height),
        }),
    )
    .into()
}

pub fn genesis() -> PlannedInteraction {
    Request::new("genesis", json!({})).into()
}

pub fn net_info() -> PlannedInteraction {
    Request::new("net_info", json!({})).into()
}

pub fn status() -> PlannedInteraction {
    Request::new("status", json!({})).into()
}

pub fn subscribe(query: &str) -> PlannedInteraction {
    PlannedSubscription::new(query).into()
}
