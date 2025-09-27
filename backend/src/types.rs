use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionId {
    pub timestamp: DateTime<Utc>,
    pub amount_cents: i64, // Store amount in cents to avoid floating point comparison issues
    pub currency: String,
    pub payee: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentTransaction {
    pub account_id: String,
    pub id: TransactionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalTransaction {
    pub account_id: String,
    pub id: TransactionId,
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTransactionRequest {
    pub account_id: String,
    pub timestamp: DateTime<Utc>,
    pub payee: String,
    pub amount: f64,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct CsvTransaction {
    pub timestamp: String, // Will be parsed into DateTime<Utc>
    pub payee: String,
    pub amount: f64,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemoRequest {
    pub memo: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BulkImportResponse {
    pub imported: usize,
    pub duplicates: usize,
    pub errors: Vec<String>,
}

pub type CurrentTransactions = Arc<Mutex<HashMap<String, HashMap<TransactionId, CurrentTransaction>>>>; // account_id -> transactions
pub type AllTransactions = Arc<Mutex<HashMap<String, Vec<HistoricalTransaction>>>>; // account_id -> transactions

#[derive(Clone)]
pub struct TransactionStore {
    pub current: CurrentTransactions,
    pub all: AllTransactions,
}

impl TransactionStore {
    pub fn new() -> Self {
        Self {
            current: Arc::new(Mutex::new(HashMap::new())),
            all: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn load_from_files(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Load current transactions
        if Path::new("current_transactions.json").exists() {
            let content = fs::read_to_string("current_transactions.json").await?;
            let data: HashMap<String, HashMap<TransactionId, CurrentTransaction>> = serde_json::from_str(&content)?;
            *self.current.lock().unwrap() = data;
        }

        // Load all transactions
        if Path::new("all_transactions.json").exists() {
            let content = fs::read_to_string("all_transactions.json").await?;
            let data: HashMap<String, Vec<HistoricalTransaction>> = serde_json::from_str(&content)?;
            *self.all.lock().unwrap() = data;
        }

        Ok(())
    }

    pub async fn save_to_files(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Save current transactions
        let current_json = {
            let current = self.current.lock().unwrap();
            serde_json::to_string_pretty(&*current)?
        };
        fs::write("current_transactions.json", current_json).await?;

        // Save all transactions
        let all_json = {
            let all = self.all.lock().unwrap();
            serde_json::to_string_pretty(&*all)?
        };
        fs::write("all_transactions.json", all_json).await?;

        Ok(())
    }
}