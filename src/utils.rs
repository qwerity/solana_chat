use chrono::{Local, Timelike};
use solana_client::{
    rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient},
    rpc_config::RpcTransactionConfig,
};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
};
use solana_transaction_status::UiTransactionEncoding;
use std::collections::HashSet;

pub fn processed_signatures(client: &RpcClient, recipient: &Pubkey) -> HashSet<String> {
    let rpc_config = GetConfirmedSignaturesForAddress2Config {
        commitment: Some(CommitmentConfig::confirmed()),
        ..Default::default()
    };

    match client.get_signatures_for_address_with_config(recipient, rpc_config) {
        Ok(signatures) => signatures.into_iter().map(|sig| sig.signature).collect(),
        Err(_) => HashSet::new(),
    }
}

pub fn current_timestamp() -> String {
    let now = Local::now();
    format!(
        "{{{:02}::{:02}::{:02}.{:03}}}",
        now.hour(),
        now.minute(),
        now.second(),
        now.timestamp_subsec_millis()
    )
}

pub fn transaction_default_config() -> RpcTransactionConfig {
    RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::Base64),
        commitment: Some(CommitmentConfig::confirmed()),
        ..Default::default()
    }
}
