use crate::error::ApiError;
use crate::store::TransactionStore;
use crate::types::CsvTransaction;
use crate::utils::{parse_csv_string, process_csv_transaction};
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

    let mut response = store.bulk_import_transactions(account_id, new_transactions).await.map_err(warp::reject::custom)?;
    response.errors = errors; // Add any parsing errors to the response

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        warp::http::StatusCode::OK,
    ))
}