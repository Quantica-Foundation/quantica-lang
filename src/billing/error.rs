use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum BillingError {
    Io(std::io::Error),
    Serialization(serde_json::Error),
    ProviderUnavailable(String),
    Validation(String),
    NotFound(String),
    Conflict(String),
}

impl Display for BillingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BillingError::Io(err) => write!(f, "I/O error: {}", err),
            BillingError::Serialization(err) => write!(f, "Serialization error: {}", err),
            BillingError::ProviderUnavailable(msg) => write!(f, "Provider unavailable: {}", msg),
            BillingError::Validation(msg) => write!(f, "Validation error: {}", msg),
            BillingError::NotFound(msg) => write!(f, "Not found: {}", msg),
            BillingError::Conflict(msg) => write!(f, "Conflict: {}", msg),
        }
    }
}

impl std::error::Error for BillingError {}

impl From<std::io::Error> for BillingError {
    fn from(value: std::io::Error) -> Self {
        BillingError::Io(value)
    }
}

impl From<serde_json::Error> for BillingError {
    fn from(value: serde_json::Error) -> Self {
        BillingError::Serialization(value)
    }
}

#[derive(Debug, Clone)]
pub enum PaymentError {
    ProviderUnavailable(String),
    Validation(String),
    Transport(String),
    Unexpected(String),
}

impl Display for PaymentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentError::ProviderUnavailable(msg) => write!(f, "Provider unavailable: {}", msg),
            PaymentError::Validation(msg) => write!(f, "Validation error: {}", msg),
            PaymentError::Transport(msg) => write!(f, "Transport error: {}", msg),
            PaymentError::Unexpected(msg) => write!(f, "Unexpected error: {}", msg),
        }
    }
}

impl std::error::Error for PaymentError {}
