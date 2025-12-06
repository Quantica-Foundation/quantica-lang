use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PaymentProviderKind {
    Stripe,
    Paypal,
    Shopify,
    Klarna,
    Affirm,
    ApplePay,
    WePay,
    Venmo,
    WeChat,
    QuickBooks,
    Mastercard,
    Visa,
    Bitcoin,
}

impl PaymentProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentProviderKind::Stripe => "stripe",
            PaymentProviderKind::Paypal => "paypal",
            PaymentProviderKind::Shopify => "shopify",
            PaymentProviderKind::Klarna => "klarna",
            PaymentProviderKind::Affirm => "affirm",
            PaymentProviderKind::ApplePay => "apple_pay",
            PaymentProviderKind::WePay => "wepay",
            PaymentProviderKind::Venmo => "venmo",
            PaymentProviderKind::WeChat => "wechat",
            PaymentProviderKind::QuickBooks => "quickbooks",
            PaymentProviderKind::Mastercard => "mastercard",
            PaymentProviderKind::Visa => "visa",
            PaymentProviderKind::Bitcoin => "bitcoin",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    RequiresAction,
    Authorized,
    Succeeded,
    Refunded,
    Failed,
    Chargeback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiTier {
    Trial,
    Standard,
    Premium,
    Enterprise,
}

impl ApiTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiTier::Trial => "trial",
            ApiTier::Standard => "standard",
            ApiTier::Premium => "premium",
            ApiTier::Enterprise => "enterprise",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentProviderConfig {
    pub provider: PaymentProviderKind,
    pub enabled: bool,
    pub api_key: Option<String>,
    pub webhook_secret: Option<String>,
    pub merchant_id: Option<String>,
    pub region: Option<String>,
}

impl PaymentProviderConfig {
    pub fn enabled(provider: PaymentProviderKind) -> Self {
        PaymentProviderConfig {
            provider,
            enabled: true,
            api_key: None,
            webhook_secret: None,
            merchant_id: None,
            region: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub provider: PaymentProviderKind,
    pub amount_cents: u64,
    pub currency: String,
    pub user_id: String,
    pub tier: ApiTier,
    pub metadata: HashMap<String, String>,
    pub return_url: Option<String>,
    pub cancel_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentIntent {
    pub id: String,
    pub provider: PaymentProviderKind,
    pub status: PaymentStatus,
    pub amount_cents: u64,
    pub currency: String,
    pub checkout_url: Option<String>,
    pub client_secret: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    pub id: String,
    pub provider: PaymentProviderKind,
    pub status: PaymentStatus,
    pub amount_cents: u64,
    pub currency: String,
    pub user_id: String,
    pub tier: ApiTier,
    pub metadata: HashMap<String, String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    pub id: String,
    pub hashed_key: String,
    pub user_id: String,
    pub payment_id: String,
    pub tier: ApiTier,
    pub created_at: u64,
    pub revoked: bool,
    pub usage_limit: Option<u64>,
    pub usage_count: u64,
    pub last_used_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BillingState {
    pub payments: Vec<PaymentRecord>,
    pub api_keys: Vec<ApiKeyRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuedApiKey {
    pub api_key: String,
    pub record: ApiKeyRecord,
}
