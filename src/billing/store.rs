use crate::billing::error::BillingError;
use crate::billing::models::{ApiKeyRecord, BillingState, PaymentRecord};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct BillingStore {
    path: PathBuf,
    state: RwLock<BillingState>,
}

impl BillingStore {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, BillingError> {
        let path = path.into();
        let state = if path.exists() {
            Self::load_state(&path)?
        } else {
            BillingState::default()
        };
        Ok(BillingStore {
            path,
            state: RwLock::new(state),
        })
    }

    pub fn read<F, R>(&self, reader: F) -> R
    where
        F: FnOnce(&BillingState) -> R,
    {
        let guard = self
            .state
            .read()
            .expect("billing state lock poisoned on read");
        reader(&guard)
    }

    pub fn write<F, R>(&self, writer: F) -> Result<R, BillingError>
    where
        F: FnOnce(&mut BillingState) -> Result<R, BillingError>,
    {
        let mut guard = self
            .state
            .write()
            .expect("billing state lock poisoned on write");
        let output = writer(&mut guard)?;
        self.persist(&guard)?;
        Ok(output)
    }

    pub fn upsert_payment(&self, record: PaymentRecord) -> Result<PaymentRecord, BillingError> {
        self.write(|state| {
            if let Some(existing) = state.payments.iter_mut().find(|item| item.id == record.id) {
                *existing = record.clone();
                return Ok(existing.clone());
            }
            state.payments.push(record.clone());
            Ok(record)
        })
    }

    pub fn upsert_api_key(&self, record: ApiKeyRecord) -> Result<ApiKeyRecord, BillingError> {
        self.write(|state| {
            if let Some(existing) = state.api_keys.iter_mut().find(|item| item.id == record.id) {
                *existing = record.clone();
                return Ok(existing.clone());
            }
            state.api_keys.push(record.clone());
            Ok(record)
        })
    }

    fn load_state(path: &Path) -> Result<BillingState, BillingError> {
        let contents = fs::read_to_string(path)?;
        let state = serde_json::from_str(&contents)?;
        Ok(state)
    }

    fn persist(&self, state: &BillingState) -> Result<(), BillingError> {
        if let Some(parent) = self.path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        let serialized = serde_json::to_vec_pretty(state)?;
        let mut file = fs::File::create(&self.path)?;
        file.write_all(&serialized)?;
        file.sync_all()?;
        Ok(())
    }
}

impl BillingStore {
    pub fn find_api_key(&self, predicate: impl Fn(&ApiKeyRecord) -> bool) -> Option<ApiKeyRecord> {
        self.read(|state| state.api_keys.iter().cloned().find(predicate))
    }

    pub fn update_api_key<F>(&self, id: &str, updater: F) -> Result<ApiKeyRecord, BillingError>
    where
        F: FnOnce(&mut ApiKeyRecord) -> Result<(), BillingError>,
    {
        self.write(|state| {
            let record = state
                .api_keys
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or_else(|| BillingError::NotFound(format!("api key {} not found", id)))?;
            updater(record)?;
            Ok(record.clone())
        })
    }

    pub fn update_payment<F>(&self, id: &str, updater: F) -> Result<PaymentRecord, BillingError>
    where
        F: FnOnce(&mut PaymentRecord) -> Result<(), BillingError>,
    {
        self.write(|state| {
            let record = state
                .payments
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or_else(|| BillingError::NotFound(format!("payment {} not found", id)))?;
            updater(record)?;
            Ok(record.clone())
        })
    }
}
