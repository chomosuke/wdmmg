use crate::error::ApiError;
use crate::types::{
    BulkImportResponse, CreateTransactionRequest, CurrentTransaction, HistoricalTransaction,
    TransactionId,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::fs;

pub type CurrentTransactions =
    Arc<Mutex<HashMap<String, HashMap<TransactionId, CurrentTransaction>>>>; // account_id -> transactions
pub type AllTransactions = Arc<Mutex<HashMap<String, Vec<HistoricalTransaction>>>>; // account_id -> transactions

#[derive(Clone)]
pub struct TransactionStore {
    current: CurrentTransactions,
    all: AllTransactions,
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
            let data: HashMap<String, HashMap<TransactionId, CurrentTransaction>> =
                serde_json::from_str(&content)?;
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

    /// Get all current transactions across all accounts
    pub fn get_current_transactions(&self) -> Vec<CurrentTransaction> {
        let current = self.current.lock().unwrap();
        let mut all_transactions = Vec::new();
        for transactions in current.values() {
            all_transactions.extend(transactions.values().cloned());
        }
        all_transactions
    }

    /// Get all historical transactions across all accounts
    pub fn get_all_transactions(&self) -> Vec<HistoricalTransaction> {
        let all = self.all.lock().unwrap();
        let mut all_transactions = Vec::new();
        for transactions in all.values() {
            all_transactions.extend(transactions.iter().cloned());
        }
        all_transactions
    }

    /// Create a new transaction
    pub async fn create_transaction(
        &self,
        request: CreateTransactionRequest,
    ) -> Result<CurrentTransaction, ApiError> {
        let transaction_id = TransactionId {
            timestamp: request.timestamp,
            amount_cents: (request.amount * 100.0).round() as i64,
            currency: request.currency,
            payee: request.payee,
        };

        let current_transaction = CurrentTransaction {
            account_id: request.account_id.clone(),
            id: transaction_id.clone(),
        };

        let historical_transaction = HistoricalTransaction {
            account_id: request.account_id.clone(),
            id: transaction_id.clone(),
            memo: None,
        };

        // Add to current transactions
        {
            let mut current = self.current.lock().unwrap();
            let account_transactions = current
                .entry(request.account_id.clone())
                .or_insert_with(HashMap::new);

            if account_transactions.contains_key(&transaction_id) {
                return Err(ApiError {
                    message: "Transaction already exists".to_string(),
                    status: warp::http::StatusCode::CONFLICT,
                });
            }

            account_transactions.insert(transaction_id.clone(), current_transaction.clone());
        }

        // Add to historical transactions
        {
            let mut all = self.all.lock().unwrap();
            let account_transactions = all.entry(request.account_id).or_insert_with(Vec::new);
            account_transactions.push(historical_transaction);
        }

        // Save to files
        if let Err(e) = self.save_to_files().await {
            eprintln!("Warning: Failed to save data: {}", e);
        }

        Ok(current_transaction)
    }

    /// Bulk import transactions from CSV data
    pub async fn bulk_import_transactions(
        &self,
        account_id: String,
        new_transactions: Vec<(TransactionId, CurrentTransaction, HistoricalTransaction)>,
    ) -> Result<BulkImportResponse, ApiError> {
        if new_transactions.is_empty() {
            return Err(ApiError {
                message: "No valid transactions to import".to_string(),
                status: warp::http::StatusCode::BAD_REQUEST,
            });
        }

        // Find date range covered by new transactions
        let mut min_date: Option<DateTime<Utc>> = None;
        let mut max_date: Option<DateTime<Utc>> = None;

        for (transaction_id, _, _) in &new_transactions {
            let date = transaction_id.timestamp;
            min_date = Some(min_date.map_or(date, |min| min.min(date)));
            max_date = Some(max_date.map_or(date, |max| max.max(date)));
        }

        let mut imported = 0;

        // Update current transactions
        {
            let mut current = self.current.lock().unwrap();
            let account_transactions = current
                .entry(account_id.clone())
                .or_insert_with(HashMap::new);

            // Remove existing transactions in the date range
            if let (Some(min_date), Some(max_date)) = (min_date, max_date) {
                account_transactions
                    .retain(|id, _| id.timestamp < min_date || id.timestamp > max_date);
            }

            // Add new transactions
            for (transaction_id, current_transaction, _) in &new_transactions {
                account_transactions.insert(transaction_id.clone(), current_transaction.clone());
                imported += 1;
            }
        }

        // Add to historical transactions
        {
            let mut all = self.all.lock().unwrap();
            let account_transactions = all.entry(account_id).or_insert_with(Vec::new);

            for (_, _, historical_transaction) in new_transactions {
                account_transactions.push(historical_transaction);
            }
        }

        // Save to files
        if let Err(e) = self.save_to_files().await {
            eprintln!("Warning: Failed to save data: {}", e);
        }

        Ok(BulkImportResponse {
            imported,
            duplicates: 0,
            errors: vec![],
        })
    }

    /// Update a transaction memo
    pub async fn update_transaction_memo(
        &self,
        account_id: String,
        transaction_id: TransactionId,
        new_memo: Option<String>,
    ) -> Result<(), ApiError> {
        // Update memo in a scope to release the lock
        {
            let mut all = self.all.lock().unwrap();
            let account_transactions = all.get_mut(&account_id).ok_or(ApiError {
                message: "Account not found".to_string(),
                status: warp::http::StatusCode::NOT_FOUND,
            })?;

            let transaction = account_transactions
                .iter_mut()
                .find(|t| t.id == transaction_id)
                .ok_or(ApiError {
                    message: "Transaction not found".to_string(),
                    status: warp::http::StatusCode::NOT_FOUND,
                })?;

            transaction.memo = new_memo;
        } // Lock is automatically dropped here

        // Save to files
        if let Err(e) = self.save_to_files().await {
            eprintln!("Warning: Failed to save data: {}", e);
        }

        Ok(())
    }
}
