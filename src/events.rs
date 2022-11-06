use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use crate::SnipeStatus;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "standard")]
#[serde(rename_all = "snake_case")]
pub enum NearEvent {
    SnipeNear(SnipeNearEvent),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SnipeNearEvent {
    pub version: String,
    #[serde(flatten)]
    pub event_kind: SnipeNearEventKind,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum SnipeNearEventKind {
    Snipe(LogSnipe),
    DeleteSnipe(LogDeleteSnipe),
    BuyToken(LogBuyToken)
}

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug)]
pub struct LogSnipe {
    pub snipe_id: u64,
    pub account_id: String,
    pub contract_id: String,
    pub token_id: Option<String>,
    pub deposit: u128,
    pub status: SnipeStatus,
}

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug)]
pub struct LogDeleteSnipe {
    pub snipe_id: u64,
    pub account_id: String,
}

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug)]
pub struct LogBuyToken {
    pub marketplace_contract_id: String,
    pub price: u128,
    pub snipe_id: u64,
    pub token_id: String,
    pub status: SnipeStatus,
}

impl Display for NearEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("EVENT_JSON:{}", self.to_json_string()))
    }
}

impl NearEvent {
    pub fn new(version: String, event_kind: SnipeNearEventKind) -> Self {
        NearEvent::SnipeNear(SnipeNearEvent {
            version,
            event_kind,
        })
    }

    pub fn new_v1(event_kind: SnipeNearEventKind) -> Self {
        NearEvent::new("1.0.0".to_string(), event_kind)
    }

    pub(crate) fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn log(&self) {
        near_sdk::env::log_str(&self.to_string());
    }

    // events

    pub fn snipe(data: LogSnipe) -> Self {
        NearEvent::new_v1(SnipeNearEventKind::Snipe(data))
    }

    pub fn delete_snipe(data: LogDeleteSnipe) -> Self {
        NearEvent::new_v1(SnipeNearEventKind::DeleteSnipe(data))
    }

    pub fn buy_token(data: LogBuyToken) -> Self {
        NearEvent::new_v1(SnipeNearEventKind::BuyToken(data))
    }

    // logs

    pub fn log_snipe(data: LogSnipe) {
        NearEvent::snipe(data).log();
    }

    pub fn log_delete_snipe(data: LogDeleteSnipe) {
        NearEvent::delete_snipe(data).log();
    }

    pub fn log_buy_token(data: LogBuyToken) {
        NearEvent::buy_token(data).log();
    }
}
