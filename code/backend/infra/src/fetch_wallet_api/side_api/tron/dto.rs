use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Trc20Response {
    #[serde(default)]
    data: Vec<Trc20Transfer>,
    #[serde(default)]
    meta: Option<Meta>,
    #[serde(default)]
    success: Option<bool>,
}

impl Trc20Response {
    /// Consume the response and return (data, meta).
    pub fn into_parts(self) -> (Vec<Trc20Transfer>, Option<Meta>) {
        (self.data, self.meta)
    }

    pub fn success(&self) -> Option<bool> {
        self.success
    }
}

#[derive(Deserialize, Debug)]
pub struct Trc20Transfer {
    transaction_id: String,
    token_info: TokenInfo,
    block_timestamp: i64,
    from: String,
    to: String,
    value: String,
    #[serde(rename = "type")]
    #[serde(default)]
    kind: Option<String>,
}

impl Trc20Transfer {
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    pub fn token_info(&self) -> &TokenInfo {
        &self.token_info
    }

    pub fn block_timestamp(&self) -> i64 {
        self.block_timestamp
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn to(&self) -> &str {
        &self.to
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }
}

#[derive(Deserialize, Debug)]
pub struct TokenInfo {
    symbol: String,
    address: String,
    decimals: u8,
    name: String,
}

impl TokenInfo {
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Deserialize, Debug)]
pub struct NativeTxResponse {
    #[serde(default)]
    data: Vec<RawTransaction>,
    #[serde(default)]
    meta: Option<Meta>,
}

impl NativeTxResponse {
    /// Consume the response and return (data, meta).
    pub fn into_parts(self) -> (Vec<RawTransaction>, Option<Meta>) {
        (self.data, self.meta)
    }
}

#[derive(Deserialize, Debug)]
pub struct RawTransaction {
    #[serde(rename = "txID")]
    tx_id: String,
    block_timestamp: i64,
    #[serde(default)]
    ret: Option<Vec<Ret>>,
    raw_data: RawData,
}

impl RawTransaction {
    pub fn tx_id(&self) -> &str {
        &self.tx_id
    }

    pub fn block_timestamp(&self) -> i64 {
        self.block_timestamp
    }

    pub fn ret(&self) -> Option<&[Ret]> {
        self.ret.as_deref()
    }

    pub fn into_raw_data(self) -> RawData {
        self.raw_data
    }
}

#[derive(Deserialize, Debug)]
pub struct Ret {
    #[serde(rename = "contractRet")]
    contract_ret: Option<String>,
}

impl Ret {
    pub fn contract_ret(&self) -> Option<&str> {
        self.contract_ret.as_deref()
    }
}

#[derive(Deserialize, Debug)]
pub struct RawData {
    contract: Vec<Contract>,
}

impl RawData {
    pub fn into_contracts(self) -> Vec<Contract> {
        self.contract
    }
}

#[derive(Deserialize, Debug)]
pub struct Contract {
    #[serde(rename = "type")]
    contract_type: String,
    parameter: Parameter,
}

impl Contract {
    pub fn contract_type(&self) -> &str {
        &self.contract_type
    }

    pub fn into_parameter(self) -> Parameter {
        self.parameter
    }
}

#[derive(Deserialize, Debug)]
pub struct Parameter {
    value: serde_json::Value,
}

impl Parameter {
    pub fn into_value(self) -> serde_json::Value {
        self.value
    }
}

#[derive(Deserialize, Debug)]
pub struct Meta {
    #[serde(default)]
    fingerprint: Option<String>,
}

impl Meta {
    pub fn fingerprint(self) -> Option<String> {
        self.fingerprint
    }
}

#[derive(Deserialize, Debug)]
pub struct TransferContractValue {
    pub amount: u128,
    pub owner_address: String,
    pub to_address: String,
}

impl TransferContractValue {
    pub fn amount(&self) -> u128 {
        self.amount
    }

    pub fn owner_address(&self) -> &str {
        &self.owner_address
    }

    pub fn to_address(&self) -> &str {
        &self.to_address
    }
}
