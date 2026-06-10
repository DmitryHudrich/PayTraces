use crate::chain::ChainId;
use crate::primitives::Address;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("unknown chain: {0:?}")]
    UnknownChain(ChainId),

    #[error("address {address} not found on chain {chain:?}")]
    AddressNotFound { address: Address, chain: ChainId },

    #[error("trace limit exceeded: {reason}")]
    TraceLimitExceeded { reason: String },

    #[error("insufficient data: {0}")]
    InsufficientData(String),

    #[error("decimals mismatch: expected {expected}, got {actual}")]
    DecimalsMismatch { expected: u8, actual: u8 },

    #[error("amount overflow")]
    AmountOverflow,
}

pub type DomainResult<T> = Result<T, DomainError>;

