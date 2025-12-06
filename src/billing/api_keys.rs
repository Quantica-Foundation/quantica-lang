use crate::billing::error::BillingError;
use crate::billing::models::{ApiKeyRecord, ApiTier, IssuedApiKey};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

const API_KEY_BYTES: usize = 32;
const SALT_BYTES: usize = 16;

pub struct ApiKeyManager {
    prefix: String,
}

impl Default for ApiKeyManager {
    fn default() -> Self {
        ApiKeyManager {
            prefix: "QNT".to_string(),
        }
    }
}

impl ApiKeyManager {
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        ApiKeyManager {
            prefix: prefix.into(),
        }
    }

    pub fn issue_key(
        &self,
        user_id: &str,
        payment_id: &str,
        tier: ApiTier,
        usage_limit: Option<u64>,
    ) -> Result<IssuedApiKey, BillingError> {
        let mut raw_key = vec![0u8; API_KEY_BYTES];
        OsRng.fill_bytes(&mut raw_key);

        let api_key = self.build_key_string(&raw_key);
        let salt = self.random_salt();
        let hashed = Self::hash_with_salt(&api_key, &salt);
        let record_id = self.random_identifier();
        let now = Self::now_epoch_seconds();

        let record = ApiKeyRecord {
            id: record_id,
            hashed_key: hashed,
            user_id: user_id.to_string(),
            payment_id: payment_id.to_string(),
            tier,
            created_at: now,
            revoked: false,
            usage_limit,
            usage_count: 0,
            last_used_at: None,
        };

        Ok(IssuedApiKey { api_key, record })
    }

    pub fn verify(&self, candidate: &str, record: &ApiKeyRecord) -> bool {
        if record.revoked {
            return false;
        }
        match Self::split_hashed_value(&record.hashed_key) {
            Some((salt, expected_digest)) => {
                let recomputed = Self::hash_digest(candidate, &salt);
                constant_time_eq::constant_time_eq(&recomputed, &expected_digest)
            }
            None => false,
        }
    }

    pub fn mark_use(record: &mut ApiKeyRecord) {
        record.usage_count = record.usage_count.saturating_add(1);
        record.last_used_at = Some(Self::now_epoch_seconds());
    }

    fn build_key_string(&self, raw: &[u8]) -> String {
        let mut hex = String::with_capacity(raw.len() * 2);
        for byte in raw {
            hex.push_str(&format!("{:02X}", byte));
        }
        let segments: Vec<String> = hex
            .as_bytes()
            .chunks(8)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or("").to_string())
            .collect();
        format!("{}-{}", self.prefix, segments.join("-"))
    }

    fn random_identifier(&self) -> String {
        let mut bytes = [0u8; 12];
        OsRng.fill_bytes(&mut bytes);
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in &bytes {
            hex.push_str(&format!("{:02x}", byte));
        }
        format!("key_{}", hex)
    }

    fn random_salt(&self) -> Vec<u8> {
        let mut salt = vec![0u8; SALT_BYTES];
        OsRng.fill_bytes(&mut salt);
        salt
    }

    fn hash_with_salt(key: &str, salt: &[u8]) -> String {
        let digest = Self::hash_digest(key, salt);
        format!("{}:${}", Self::to_hex(salt), Self::to_hex(&digest))
    }

    fn hash_digest(key: &str, salt: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(salt);
        hasher.update(key.as_bytes());
        hasher.finalize().to_vec()
    }

    fn split_hashed_value(encoded: &str) -> Option<(Vec<u8>, Vec<u8>)> {
        let mut parts = encoded.splitn(2, ':');
        let salt_hex = parts.next()?;
        let digest_hex = parts.next()?.trim_start_matches('$');
        Some((Self::from_hex(salt_hex)?, Self::from_hex(digest_hex)?))
    }

    fn to_hex(bytes: &[u8]) -> String {
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            hex.push_str(&format!("{:02x}", byte));
        }
        hex
    }

    fn from_hex(input: &str) -> Option<Vec<u8>> {
        if input.len() % 2 != 0 {
            return None;
        }
        let mut output = Vec::with_capacity(input.len() / 2);
        let chars: Vec<char> = input.chars().collect();
        for pair in chars.chunks(2) {
            let hi = pair.get(0)?.to_digit(16)?;
            let lo = pair.get(1)?.to_digit(16)?;
            output.push((hi << 4 | lo) as u8);
        }
        Some(output)
    }

    fn now_epoch_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

mod constant_time_eq {
    pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut diff = 0u8;
        for (&x, &y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }
}
