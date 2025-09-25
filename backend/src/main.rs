use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use warp::Filter;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use csv::Reader;
use std::io::Cursor;
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

type CurrentTransactions = Arc<Mutex<HashMap<String, HashMap<TransactionId, CurrentTransaction>>>>; // account_id -> transactions
type AllTransactions = Arc<Mutex<HashMap<String, Vec<HistoricalTransaction>>>>; // account_id -> transactions

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

#[tokio::main]
async fn main() {
    let store = TransactionStore::new();

    // Load existing data from files
    if let Err(e) = store.load_from_files().await {
        eprintln!("Warning: Failed to load existing data: {}", e);
    }

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST"]);

    // GET /transactions/current - Get current transactions
    let get_current_transactions = warp::path!("transactions" / "current")
        .and(warp::get())
        .and(with_store(store.clone()))
        .and_then(get_current_transactions_handler);

    // GET /transactions/all - Get all historical transactions
    let get_all_transactions = warp::path!("transactions" / "all")
        .and(warp::get())
        .and(with_store(store.clone()))
        .and_then(get_all_transactions_handler);

    // POST /transactions - Create a new transaction
    let create_transaction = warp::path("transactions")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_store(store.clone()))
        .and_then(create_transaction_handler);

    // POST /transactions/bulk/:account_id - Upload CSV for bulk import
    let bulk_import = warp::path!("transactions" / "bulk" / String)
        .and(warp::post())
        .and(warp::body::bytes())
        .and(with_store(store.clone()))
        .and_then(bulk_import_handler);

    // PUT /transactions/:account_id/memo - Update transaction memo
    let update_memo = warp::path!("transactions" / String / "memo")
        .and(warp::put())
        .and(warp::body::json())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(with_store(store.clone()))
        .and_then(update_memo_handler);

    let routes = get_current_transactions
        .or(get_all_transactions)
        .or(create_transaction)
        .or(bulk_import)
        .or(update_memo)
        .with(cors);

    println!("Server running on http://localhost:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

fn with_store(
    store: TransactionStore,
) -> impl Filter<Extract = (TransactionStore,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || store.clone())
}

async fn get_current_transactions_handler(
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let current = store.current.lock().unwrap();
    let mut all_transactions = Vec::new();
    for transactions in current.values() {
        all_transactions.extend(transactions.values().cloned());
    }
    Ok(warp::reply::json(&all_transactions))
}

async fn get_all_transactions_handler(
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let all = store.all.lock().unwrap();
    let mut all_transactions = Vec::new();
    for transactions in all.values() {
        all_transactions.extend(transactions.iter().cloned());
    }
    Ok(warp::reply::json(&all_transactions))
}

async fn create_transaction_handler(
    request: CreateTransactionRequest,
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
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

    {
        let mut current = store.current.lock().unwrap();
        let account_transactions = current.entry(request.account_id.clone()).or_insert_with(HashMap::new);

        if account_transactions.contains_key(&transaction_id) {
            return Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({"error": "Transaction already exists"})),
                warp::http::StatusCode::CONFLICT,
            ));
        }

        account_transactions.insert(transaction_id.clone(), current_transaction.clone());
    }

    {
        let mut all = store.all.lock().unwrap();
        let account_transactions = all.entry(request.account_id).or_insert_with(Vec::new);
        account_transactions.push(historical_transaction);
    }

    // Save to files
    if let Err(e) = store.save_to_files().await {
        eprintln!("Warning: Failed to save data: {}", e);
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&current_transaction),
        warp::http::StatusCode::CREATED,
    ))
}

async fn bulk_import_handler(
    account_id: String,
    csv_data: bytes::Bytes,
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let csv_string = match String::from_utf8(csv_data.to_vec()) {
        Ok(s) => s,
        Err(_) => return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "Invalid UTF-8 in CSV"})),
            warp::http::StatusCode::BAD_REQUEST,
        )),
    };

    let cursor = Cursor::new(csv_string);
    let mut reader = Reader::from_reader(cursor);

    let mut new_transactions = Vec::new();
    let mut errors = Vec::new();

    // Parse CSV records
    for (row_idx, result) in reader.deserialize::<CsvTransaction>().enumerate() {
        match result {
            Ok(csv_transaction) => {
                match csv_transaction.timestamp.parse::<DateTime<Utc>>() {
                    Ok(timestamp) => {
                        let transaction_id = TransactionId {
                            timestamp,
                            amount_cents: (csv_transaction.amount * 100.0).round() as i64,
                            currency: csv_transaction.currency,
                            payee: csv_transaction.payee,
                        };

                        let current_transaction = CurrentTransaction {
                            account_id: account_id.clone(),
                            id: transaction_id.clone(),
                        };

                        let historical_transaction = HistoricalTransaction {
                            account_id: account_id.clone(),
                            id: transaction_id.clone(),
                            memo: None,
                        };

                        new_transactions.push((transaction_id, current_transaction, historical_transaction));
                    },
                    Err(e) => {
                        errors.push(format!("Row {}: Invalid timestamp format - {}", row_idx + 2, e));
                    }
                }
            },
            Err(e) => {
                errors.push(format!("Row {}: CSV parsing error - {}", row_idx + 2, e));
            }
        }
    }

    if new_transactions.is_empty() && !errors.is_empty() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&BulkImportResponse {
                imported: 0,
                duplicates: 0,
                errors,
            }),
            warp::http::StatusCode::BAD_REQUEST,
        ));
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
        let mut current = store.current.lock().unwrap();
        let account_transactions = current.entry(account_id.clone()).or_insert_with(HashMap::new);

        // Remove existing transactions in the date range
        if let (Some(min_date), Some(max_date)) = (min_date, max_date) {
            account_transactions.retain(|id, _| {
                id.timestamp < min_date || id.timestamp > max_date
            });
        }

        // Add new transactions
        for (transaction_id, current_transaction, _) in &new_transactions {
            account_transactions.insert(transaction_id.clone(), current_transaction.clone());
            imported += 1;
        }
    }

    // Add to historical transactions
    {
        let mut all = store.all.lock().unwrap();
        let account_transactions = all.entry(account_id).or_insert_with(Vec::new);

        for (_, _, historical_transaction) in new_transactions {
            account_transactions.push(historical_transaction);
        }
    }

    // Save to files
    if let Err(e) = store.save_to_files().await {
        eprintln!("Warning: Failed to save data: {}", e);
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&BulkImportResponse {
            imported,
            duplicates: 0,
            errors,
        }),
        warp::http::StatusCode::OK,
    ))
}

async fn update_memo_handler(
    account_id: String,
    memo_request: UpdateMemoRequest,
    query_params: std::collections::HashMap<String, String>,
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Extract transaction parameters from query
    let timestamp_str = match query_params.get("timestamp") {
        Some(s) => s,
        None => return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "Missing timestamp parameter"})),
            warp::http::StatusCode::BAD_REQUEST,
        )),
    };

    let amount_str = match query_params.get("amount") {
        Some(s) => s,
        None => return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "Missing amount parameter"})),
            warp::http::StatusCode::BAD_REQUEST,
        )),
    };

    let currency = match query_params.get("currency") {
        Some(s) => s,
        None => return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "Missing currency parameter"})),
            warp::http::StatusCode::BAD_REQUEST,
        )),
    };

    let payee = match query_params.get("payee") {
        Some(s) => s,
        None => return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "Missing payee parameter"})),
            warp::http::StatusCode::BAD_REQUEST,
        )),
    };

    let timestamp: DateTime<Utc> = match timestamp_str.parse() {
        Ok(t) => t,
        Err(_) => return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "Invalid timestamp format"})),
            warp::http::StatusCode::BAD_REQUEST,
        )),
    };

    let amount: f64 = match amount_str.parse() {
        Ok(a) => a,
        Err(_) => return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({"error": "Invalid amount format"})),
            warp::http::StatusCode::BAD_REQUEST,
        )),
    };

    let transaction_id = TransactionId {
        timestamp,
        amount_cents: (amount * 100.0).round() as i64,
        currency: currency.clone(),
        payee: payee.clone(),
    };

    // Update memo in a scope to release the lock
    {
        let mut all = store.all.lock().unwrap();
        let account_transactions = match all.get_mut(&account_id) {
            Some(transactions) => transactions,
            None => return Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({"error": "Account not found"})),
                warp::http::StatusCode::NOT_FOUND,
            )),
        };

        let transaction = match account_transactions.iter_mut().find(|t| t.id == transaction_id) {
            Some(t) => t,
            None => return Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({"error": "Transaction not found"})),
                warp::http::StatusCode::NOT_FOUND,
            )),
        };

        transaction.memo = memo_request.memo;
    } // Lock is automatically dropped here

    // Save to files
    if let Err(e) = store.save_to_files().await {
        eprintln!("Warning: Failed to save data: {}", e);
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({"message": "Memo updated successfully"})),
        warp::http::StatusCode::OK,
    ))
}
