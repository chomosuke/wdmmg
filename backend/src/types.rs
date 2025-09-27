use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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