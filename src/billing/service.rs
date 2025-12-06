use crate::billing::api_keys::ApiKeyManager;
use crate::billing::error::BillingError;
use crate::billing::models::{
    ApiKeyRecord, ApiTier, BillingState, IssuedApiKey, PaymentIntent, PaymentProviderConfig,
    PaymentProviderKind, PaymentRecord, PaymentRequest, PaymentStatus,
};
use crate::billing::providers::{build_processor_map, PaymentProcessor};
use crate::billing::store::BillingStore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct BillingService {
    store: Arc<BillingStore>,
    processors: HashMap<PaymentProviderKind, Arc<dyn PaymentProcessor>>,
    key_manager: ApiKeyManager,
}

static GLOBAL_BILLING: OnceLock<Arc<BillingService>> = OnceLock::new();

impl BillingService {
    pub fn new(
        store_path: impl Into<PathBuf>,
        provider_configs: Vec<PaymentProviderConfig>,
    ) -> Result<Self, BillingError> {
        let store = Arc::new(BillingStore::new(store_path)?);
        let processors = build_processor_map(provider_configs);
        Ok(BillingService {
            store,
            processors,
            key_manager: ApiKeyManager::default(),
        })
    }

    pub fn init_global(service: BillingService) -> Arc<BillingService> {
        let arc = Arc::new(service);
        let _ = GLOBAL_BILLING.set(arc.clone());
        arc
    }

    pub fn global() -> Arc<BillingService> {
        GLOBAL_BILLING
            .get_or_init(|| {
                Arc::new(
                    BillingService::new(
                        BillingService::default_store_path(),
                        BillingService::default_provider_configs(),
                    )
                    .expect("failed to initialise default billing service"),
                )
            })
            .clone()
    }

    fn default_store_path() -> PathBuf {
        PathBuf::from(".quantica/billing_state.json")
    }

    fn default_provider_configs() -> Vec<PaymentProviderConfig> {
        vec![
            PaymentProviderConfig::enabled(PaymentProviderKind::Stripe),
            PaymentProviderConfig::enabled(PaymentProviderKind::Paypal),
            PaymentProviderConfig::enabled(PaymentProviderKind::Shopify),
            PaymentProviderConfig::enabled(PaymentProviderKind::Klarna),
            PaymentProviderConfig::enabled(PaymentProviderKind::Affirm),
            PaymentProviderConfig::enabled(PaymentProviderKind::ApplePay),
            PaymentProviderConfig::enabled(PaymentProviderKind::WePay),
            PaymentProviderConfig::enabled(PaymentProviderKind::Venmo),
            PaymentProviderConfig::enabled(PaymentProviderKind::WeChat),
            PaymentProviderConfig::enabled(PaymentProviderKind::QuickBooks),
            PaymentProviderConfig::enabled(PaymentProviderKind::Mastercard),
            PaymentProviderConfig::enabled(PaymentProviderKind::Visa),
            PaymentProviderConfig::enabled(PaymentProviderKind::Bitcoin),
        ]
    }

    pub fn create_checkout(&self, request: PaymentRequest) -> Result<PaymentIntent, BillingError> {
        let processor = self
            .processors
            .get(&request.provider)
            .ok_or_else(|| {
                BillingError::ProviderUnavailable(request.provider.as_str().to_string())
            })?
            .clone();

        let intent = processor
            .create_payment_intent(&request)
            .map_err(|err| BillingError::Validation(err.to_string()))?;

        let now = Self::now_epoch_seconds();
        let record = PaymentRecord {
            id: intent.id.clone(),
            provider: request.provider.clone(),
            status: intent.status.clone(),
            amount_cents: request.amount_cents,
            currency: request.currency.clone(),
            user_id: request.user_id.clone(),
            tier: request.tier.clone(),
            metadata: intent.metadata.clone(),
            created_at: now,
            updated_at: now,
            reference: None,
        };
        self.store.upsert_payment(record)?;

        Ok(intent)
    }

    pub fn settle_payment(
        &self,
        payment_id: &str,
        reference: Option<String>,
        usage_limit: Option<u64>,
    ) -> Result<IssuedApiKey, BillingError> {
        let mut payment = self.store.update_payment(payment_id, |record| {
            record.status = PaymentStatus::Succeeded;
            record.updated_at = Self::now_epoch_seconds();
            record.reference = reference.clone();
            Ok(())
        })?;

        let issued = self.key_manager.issue_key(
            &payment.user_id,
            &payment.id,
            payment.tier.clone(),
            usage_limit,
        )?;
        payment
            .metadata
            .insert("api_key_id".to_string(), issued.record.id.clone());
        self.store.upsert_payment(payment)?;
        self.store.upsert_api_key(issued.record.clone())?;
        Ok(issued)
    }

    pub fn mark_payment_failed(&self, payment_id: &str, reason: &str) -> Result<(), BillingError> {
        self.store.update_payment(payment_id, |record| {
            record.status = PaymentStatus::Failed;
            record
                .metadata
                .insert("failure_reason".to_string(), reason.to_string());
            record.updated_at = Self::now_epoch_seconds();
            Ok(())
        })?;
        Ok(())
    }

    pub fn validate_api_key(&self, candidate: &str) -> Result<ApiKeyRecord, BillingError> {
        let record_id = self.store.read(|state| {
            state
                .api_keys
                .iter()
                .find(|record| self.key_manager.verify(candidate, record))
                .map(|record| record.id.clone())
        });

        let record_id = record_id
            .ok_or_else(|| BillingError::Validation("invalid or unknown API key".to_string()))?;

        self.store.update_api_key(&record_id, |record| {
            if record.revoked {
                return Err(BillingError::Validation(
                    "API key has been revoked".to_string(),
                ));
            }
            if let Some(limit) = record.usage_limit {
                if record.usage_count >= limit {
                    return Err(BillingError::Validation(
                        "API key usage limit reached".to_string(),
                    ));
                }
            }
            ApiKeyManager::mark_use(record);
            Ok(())
        })
    }

    pub fn revoke_api_key(&self, record_id: &str) -> Result<ApiKeyRecord, BillingError> {
        self.store.update_api_key(record_id, |record| {
            record.revoked = true;
            record.last_used_at = Some(Self::now_epoch_seconds());
            Ok(())
        })
    }

    pub fn list_state(&self) -> BillingState {
        self.store.read(|state| state.clone())
    }

    pub fn processors(&self) -> Vec<PaymentProviderKind> {
        self.processors.keys().cloned().collect()
    }

    fn now_epoch_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}
