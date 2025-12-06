use crate::billing::error::PaymentError;
use crate::billing::models::{
    PaymentIntent, PaymentProviderConfig, PaymentProviderKind, PaymentRequest, PaymentStatus,
};
use rand::rngs::OsRng;
use rand::RngCore;
use std::collections::HashMap;
use std::sync::Arc;

pub trait PaymentProcessor: Send + Sync {
    fn kind(&self) -> PaymentProviderKind;
    fn create_payment_intent(
        &self,
        request: &PaymentRequest,
    ) -> Result<PaymentIntent, PaymentError>;
    fn confirm_intent(&self, intent_id: &str) -> Result<PaymentStatus, PaymentError>;
    fn validate_webhook_signature(&self, signature: &str, payload: &[u8]) -> bool;
}

pub struct HostedCheckoutProcessor {
    config: PaymentProviderConfig,
}

impl HostedCheckoutProcessor {
    pub fn new(config: PaymentProviderConfig) -> Self {
        HostedCheckoutProcessor { config }
    }

    fn checkout_domain(&self) -> &'static str {
        match self.config.provider {
            PaymentProviderKind::Stripe => "checkout.stripe.com",
            PaymentProviderKind::Paypal => "www.paypal.com",
            PaymentProviderKind::Shopify => "shop.payments.shopify.com",
            PaymentProviderKind::Klarna => "pay.klarna.com",
            PaymentProviderKind::Affirm => "checkout.affirm.com",
            PaymentProviderKind::ApplePay => "pay.apple.com",
            PaymentProviderKind::WePay => "go.wepay.com",
            PaymentProviderKind::Venmo => "pay.venmo.com",
            PaymentProviderKind::WeChat => "pay.wechat.com",
            PaymentProviderKind::QuickBooks => "payments.quickbooks.com",
            PaymentProviderKind::Mastercard => "checkout.mastercard.com",
            PaymentProviderKind::Visa => "secure.visa.com",
            PaymentProviderKind::Bitcoin => "pay.bitcoin.example",
        }
    }

    fn create_reference(&self) -> String {
        let mut bytes = [0u8; 10];
        OsRng.fill_bytes(&mut bytes);
        let mut reference = String::with_capacity(bytes.len() * 2);
        for byte in &bytes {
            reference.push_str(&format!("{:02X}", byte));
        }
        format!("{}_{}", self.config.provider.as_str(), reference)
    }
}

impl PaymentProcessor for HostedCheckoutProcessor {
    fn kind(&self) -> PaymentProviderKind {
        self.config.provider.clone()
    }

    fn create_payment_intent(
        &self,
        request: &PaymentRequest,
    ) -> Result<PaymentIntent, PaymentError> {
        if !self.config.enabled {
            return Err(PaymentError::ProviderUnavailable(format!(
                "{} is disabled",
                self.config.provider.as_str()
            )));
        }

        if request.amount_cents == 0 {
            return Err(PaymentError::Validation(
                "amount must be greater than zero".to_string(),
            ));
        }

        let intent_id = self.create_reference();
        let mut metadata = request.metadata.clone();
        metadata.insert("user_id".to_string(), request.user_id.clone());
        metadata.insert("tier".to_string(), request.tier.as_str().to_string());

        let checkout_url = request.return_url.clone().or_else(|| {
            Some(format!(
                "https://{}/checkout?intent={}",
                self.checkout_domain(),
                intent_id
            ))
        });

        Ok(PaymentIntent {
            id: intent_id,
            provider: self.config.provider.clone(),
            status: PaymentStatus::Pending,
            amount_cents: request.amount_cents,
            currency: request.currency.clone(),
            checkout_url,
            client_secret: None,
            metadata,
        })
    }

    fn confirm_intent(&self, _intent_id: &str) -> Result<PaymentStatus, PaymentError> {
        if !self.config.enabled {
            return Err(PaymentError::ProviderUnavailable(format!(
                "{} is disabled",
                self.config.provider.as_str()
            )));
        }
        Ok(PaymentStatus::Succeeded)
    }

    fn validate_webhook_signature(&self, signature: &str, payload: &[u8]) -> bool {
        if let Some(expected) = &self.config.webhook_secret {
            secure_compare(signature.as_bytes(), expected.as_bytes()) && !payload.is_empty()
        } else {
            true
        }
    }
}

pub fn build_processor_map(
    configs: Vec<PaymentProviderConfig>,
) -> HashMap<PaymentProviderKind, Arc<dyn PaymentProcessor>> {
    let mut map: HashMap<PaymentProviderKind, Arc<dyn PaymentProcessor>> = HashMap::new();
    for config in configs {
        map.insert(
            config.provider.clone(),
            Arc::new(HostedCheckoutProcessor::new(config)),
        );
    }
    map
}

fn secure_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
