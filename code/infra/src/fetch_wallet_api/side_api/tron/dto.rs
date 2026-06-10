use serde::Deserialize;

#[derive(Deserialize)]
pub struct Trc20Response {
    pub data: Vec<Trc20Transfer>,
    pub meta: Option<Meta>,
}

#[derive(Deserialize)]
pub struct Trc20Transfer {
    pub transaction_id: String,
    pub token_info: TokenInfo,
    pub block_timestamp: u64,
    pub from: String,
    pub to: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub name: String,
}

#[derive(Deserialize)]
pub struct NativeTxResponse {
    pub data: Vec<RawTransaction>,
    pub meta: Option<Meta>,
}

#[derive(Deserialize)]
pub struct RawTransaction {
    #[serde(rename = "txID")]
    pub tx_id: String,
    pub block_timestamp: u64,
    pub ret: Option<Vec<Ret>>,
    pub raw_data: RawData,
}

#[derive(Deserialize)]
pub struct Ret {
    #[serde(rename = "contractRet")]
    pub contract_ret: Option<String>,
}

#[derive(Deserialize)]
pub struct RawData {
    pub contract: Vec<Contract>,
}

#[derive(Deserialize)]
pub struct Contract {
    #[serde(rename = "type")]
    pub contract_type: String,
    pub parameter: Parameter,
}

#[derive(Deserialize)]
pub struct Parameter {
    pub value: serde_json::Value,
}

#[derive(Deserialize)]
pub struct Meta {
    pub fingerprint: Option<String>,
}
