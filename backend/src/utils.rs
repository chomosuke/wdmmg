use crate::error::ApiError;
use crate::types::*;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use warp::{self, Filter};

pub fn get_required_param(params: &HashMap<String, String>, key: &str) -> Result<String, ApiError> {
    params.get(key).cloned().ok_or(ApiError {
        message: format!("Missing {} parameter", key),
        status: warp::http::StatusCode::BAD_REQUEST,
    })
}

pub fn parse_timestamp(timestamp_str: &str) -> Result<DateTime<Utc>, ApiError> {
    timestamp_str.parse().map_err(|_| ApiError {
        message: "Invalid timestamp format".to_string(),
        status: warp::http::StatusCode::BAD_REQUEST,
    })
}

pub fn parse_amount(amount_str: &str) -> Result<f64, ApiError> {
    amount_str.parse().map_err(|_| ApiError {
        message: "Invalid amount format".to_string(),
        status: warp::http::StatusCode::BAD_REQUEST,
    })
}

pub fn parse_csv_string(csv_data: bytes::Bytes) -> Result<String, ApiError> {
    String::from_utf8(csv_data.to_vec()).map_err(|_| ApiError {
        message: "Invalid UTF-8 in CSV".to_string(),
        status: warp::http::StatusCode::BAD_REQUEST,
    })
}

pub fn process_csv_transaction(
    csv_transaction: CsvTransaction,
    account_id: &str,
) -> Result<(TransactionId, CurrentTransaction, HistoricalTransaction), String> {
    let timestamp = csv_transaction.timestamp.parse::<DateTime<Utc>>()
        .map_err(|e| format!("Invalid timestamp format - {}", e))?;

    let transaction_id = TransactionId {
        timestamp,
        amount_cents: (csv_transaction.amount * 100.0).round() as i64,
        currency: csv_transaction.currency,
        payee: csv_transaction.payee,
    };

    let current_transaction = CurrentTransaction {
        account_id: account_id.to_string(),
        id: transaction_id.clone(),
    };

    let historical_transaction = HistoricalTransaction {
        account_id: account_id.to_string(),
        id: transaction_id.clone(),
        memo: None,
    };

    Ok((transaction_id, current_transaction, historical_transaction))
}

pub fn with_store(
    store: TransactionStore,
) -> impl warp::Filter<Extract = (TransactionStore,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || store.clone())
}