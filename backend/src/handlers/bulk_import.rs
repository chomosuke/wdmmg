use crate::error::ApiError;
use crate::types::*;
use crate::utils::{parse_csv_string, process_csv_transaction};
use chrono::{DateTime, Utc};
use csv::Reader;
use std::io::Cursor;
use warp;

pub async fn bulk_import_handler(
    account_id: String,
    csv_data: bytes::Bytes,
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let csv_string = parse_csv_string(csv_data).map_err(warp::reject::custom)?;

    let cursor = Cursor::new(csv_string);
    let mut reader = Reader::from_reader(cursor);

    // Parse CSV records
    let (successes, failures): (Vec<_>, Vec<_>) = reader
        .deserialize::<CsvTransaction>()
        .enumerate()
        .map(|(row_idx, result)| {
            result
                .map_err(|e| format!("Row {}: CSV parsing error - {}", row_idx + 2, e))
                .and_then(|tx| process_csv_transaction(tx, &account_id)
                    .map_err(|e| format!("Row {}: {}", row_idx + 2, e)))
        })
        .partition(Result::is_ok);

    let new_transactions: Vec<_> = successes.into_iter().map(Result::unwrap).collect();
    let errors: Vec<_> = failures.into_iter().map(Result::unwrap_err).collect();

    if new_transactions.is_empty() && !errors.is_empty() {
        return Err(warp::reject::custom(ApiError {
            message: format!("CSV parsing failed with {} errors", errors.len()),
            status: warp::http::StatusCode::BAD_REQUEST,
        }));
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
        let account_transactions = current.entry(account_id.clone()).or_insert_with(std::collections::HashMap::new);

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