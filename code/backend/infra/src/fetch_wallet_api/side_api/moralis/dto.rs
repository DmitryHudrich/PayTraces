#![allow(unused)]

use anyhow::Context;
use anyhow::anyhow;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct WalletHistoryResponse {
    page: u32,
    page_size: u32,
    cursor: Option<String>,
    result: Vec<WalletHistoryTransaction>,
}

impl WalletHistoryResponse {
    pub fn page(&self) -> u32 {
        self.page
    }

    pub fn page_size(&self) -> u32 {
        self.page_size
    }

    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    pub fn result(self) -> Vec<WalletHistoryTransaction> {
        self.result
    }
}

#[derive(Deserialize)]
pub struct DateToBlockResponse {
    pub block: u64,
    pub timestamp: u64,
}

impl DateToBlockResponse {
    pub fn block(&self) -> u64 {
        self.block
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

#[derive(Deserialize)]
pub struct WalletHistoryTransaction {
    hash: String,
    nonce: String,
    from_address: String,
    to_address: Option<String>,
    value: String,
    gas_price: Option<String>,
    receipt_gas_used: Option<String>,
    receipt_status: Option<String>,
    block_timestamp: String,
    block_number: String,
    summary: Option<String>,
    category: Option<String>,
    method_label: Option<String>,
    transaction_fee: Option<String>,
    possible_spam: Option<bool>,
    #[serde(default)]
    erc20_transfers: Vec<Erc20Transfer>,
    #[serde(default)]
    native_transfers: Vec<NativeTransfer>,
    #[serde(default)]
    nft_transfers: Vec<NftTransfer>,
}

impl WalletHistoryTransaction {
    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn from_address(&self) -> &str {
        &self.from_address
    }

    pub fn to_address(&self) -> Option<&str> {
        self.to_address.as_deref()
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn receipt_status(&self) -> Option<&str> {
        self.receipt_status.as_deref()
    }

    pub fn block_timestamp(&self) -> &str {
        &self.block_timestamp
    }

    pub fn block_number(&self) -> &str {
        &self.block_number
    }

    pub fn erc20_transfers(&self) -> &[Erc20Transfer] {
        &self.erc20_transfers
    }

    pub fn native_transfers(&self) -> &[NativeTransfer] {
        &self.native_transfers
    }
}

#[derive(Deserialize)]
pub struct Erc20Transfer {
    address: Option<String>,
    from_address: Option<String>,
    to_address: Option<String>,
    value: Option<String>,
    value_formatted: Option<String>,
    token_name: Option<String>,
    token_symbol: Option<String>,
    token_decimals: Option<String>,
    log_index: Option<u64>,
    possible_spam: Option<bool>,
    verified_contract: Option<bool>,
    direction: Option<String>,
}

impl Erc20Transfer {
    pub fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }

    pub fn from_address(&self) -> Option<&str> {
        self.from_address.as_deref()
    }

    pub fn to_address(&self) -> Option<&str> {
        self.to_address.as_deref()
    }

    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn token_decimals(&self) -> Option<&str> {
        self.token_decimals.as_deref()
    }

    pub fn log_index(&self) -> Option<u64> {
        self.log_index
    }
}

#[derive(Deserialize)]
pub struct NativeTransfer {
    from_address: Option<String>,
    to_address: Option<String>,
    value: Option<String>,
    value_formatted: Option<String>,
    direction: Option<String>,
    internal_transaction: Option<bool>,
    token_symbol: Option<String>,
}

impl NativeTransfer {
    pub fn from_address(&self) -> Option<&str> {
        self.from_address.as_deref()
    }

    pub fn to_address(&self) -> Option<&str> {
        self.to_address.as_deref()
    }

    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }
}

#[derive(Deserialize)]
pub struct NftTransfer {
    token_address: Option<String>,
    token_id: Option<String>,
    from_address: Option<String>,
    to_address: Option<String>,
    contract_type: Option<String>,
    direction: Option<String>,
    token_name: Option<String>,
    token_symbol: Option<String>,
    possible_spam: Option<bool>,
    verified_collection: Option<bool>,
}

#[derive(Deserialize)]
pub struct WalletTransactionsResponse {
    cursor: Option<String>,
    result: Vec<WalletTransaction>,
}

impl WalletTransactionsResponse {
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    pub fn into_result(self) -> Vec<WalletTransaction> {
        self.result
    }
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

impl WalletTransaction {
    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn from_address(&self) -> &str {
        &self.from_address
    }

    pub fn to_address(&self) -> Option<&str> {
        self.to_address.as_deref()
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn receipt_status(&self) -> Option<&str> {
        self.receipt_status.as_deref()
    }

    pub fn block_timestamp(&self) -> &str {
        &self.block_timestamp
    }

    pub fn block_number(&self) -> &str {
        &self.block_number
    }

    pub fn block_hash(&self) -> Option<&str> {
        self.block_hash.as_deref()
    }
}

#[derive(Deserialize)]
pub struct Erc20TransfersResponse {
    cursor: Option<String>,
    result: Vec<Erc20TransferRecord>,
}

impl Erc20TransfersResponse {
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    pub fn into_result(self) -> Vec<Erc20TransferRecord> {
        self.result
    }
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

impl Erc20TransferRecord {
    pub fn transaction_hash(&self) -> &str {
        &self.transaction_hash
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn from_address(&self) -> &str {
        &self.from_address
    }

    pub fn to_address(&self) -> &str {
        &self.to_address
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn token_decimals(&self) -> Option<&str> {
        self.token_decimals.as_deref()
    }

    pub fn log_index(&self) -> Option<u64> {
        self.log_index
    }

    pub fn block_timestamp(&self) -> &str {
        &self.block_timestamp
    }

    pub fn block_number(&self) -> &str {
        &self.block_number
    }

    pub fn block_hash(&self) -> Option<&str> {
        self.block_hash.as_deref()
    }
}
