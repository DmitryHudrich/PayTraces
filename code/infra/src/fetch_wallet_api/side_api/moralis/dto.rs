#![allow(unused)]

use anyhow::Context;
use anyhow::anyhow;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct WalletHistoryResponse {
    pub page: u32,
    pub page_size: u32,
    pub cursor: Option<String>,
    pub result: Vec<WalletHistoryTransaction>,
}

#[derive(Deserialize)]
pub struct DateToBlockResponse {
    pub block: u64,
    pub timestamp: u64,
}

#[derive(Deserialize)]
pub struct WalletHistoryTransaction {
    pub hash: String,
    pub nonce: String,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: String,
    pub gas_price: Option<String>,
    pub receipt_gas_used: Option<String>,
    pub receipt_status: Option<String>,
    pub block_timestamp: String,
    pub block_number: String,
    pub summary: Option<String>,
    pub category: Option<String>,
    pub method_label: Option<String>,
    pub transaction_fee: Option<String>,
    pub possible_spam: Option<bool>,
    #[serde(default)]
    pub erc20_transfers: Vec<Erc20Transfer>,
    #[serde(default)]
    pub native_transfers: Vec<NativeTransfer>,
    #[serde(default)]
    pub nft_transfers: Vec<NftTransfer>,
}

#[derive(Deserialize)]
pub struct Erc20Transfer {
    pub address: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub value: Option<String>,
    pub value_formatted: Option<String>,
    pub token_name: Option<String>,
    pub token_symbol: Option<String>,
    pub token_decimals: Option<String>,
    pub log_index: Option<u64>,
    pub possible_spam: Option<bool>,
    pub verified_contract: Option<bool>,
    pub direction: Option<String>,
}

#[derive(Deserialize)]
pub struct NativeTransfer {
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub value: Option<String>,
    pub value_formatted: Option<String>,
    pub direction: Option<String>,
    pub internal_transaction: Option<bool>,
    pub token_symbol: Option<String>,
}

#[derive(Deserialize)]
pub struct NftTransfer {
    pub token_address: Option<String>,
    pub token_id: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub contract_type: Option<String>,
    pub direction: Option<String>,
    pub token_name: Option<String>,
    pub token_symbol: Option<String>,
    pub possible_spam: Option<bool>,
    pub verified_collection: Option<bool>,
}

#[derive(Deserialize)]
pub struct WalletTransactionsResponse {
    pub cursor: Option<String>,
    pub result: Vec<WalletTransaction>,
}

#[derive(Deserialize)]
pub struct WalletTransaction {
    pub hash: String,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: String,
    pub receipt_status: Option<String>,
    pub block_timestamp: String,
    pub block_number: String,
    pub block_hash: Option<String>,
}

#[derive(Deserialize)]
pub struct Erc20TransfersResponse {
    pub cursor: Option<String>,
    pub result: Vec<Erc20TransferRecord>,
}

#[derive(Deserialize)]
pub struct Erc20TransferRecord {
    pub transaction_hash: String,
    pub address: String,
    pub from_address: String,
    pub to_address: String,
    pub value: String,
    pub token_decimals: Option<String>,
    pub log_index: Option<u64>,
    pub block_timestamp: String,
    pub block_number: String,
    pub block_hash: Option<String>,
}
