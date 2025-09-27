use crate::error::ApiError;
use crate::types::*;
use crate::utils::{get_required_param, parse_timestamp, parse_amount};
use std::collections::HashMap;
use warp;

pub async fn update_memo_handler(
    account_id: String,
    memo_request: UpdateMemoRequest,
    query_params: HashMap<String, String>,
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let timestamp_str = get_required_param(&query_params, "timestamp").map_err(warp::reject::custom)?;
    let amount_str = get_required_param(&query_params, "amount").map_err(warp::reject::custom)?;
    let currency = get_required_param(&query_params, "currency").map_err(warp::reject::custom)?;
    let payee = get_required_param(&query_params, "payee").map_err(warp::reject::custom)?;

    let timestamp = parse_timestamp(&timestamp_str).map_err(warp::reject::custom)?;
    let amount = parse_amount(&amount_str).map_err(warp::reject::custom)?;

    let transaction_id = TransactionId {
        timestamp,
        amount_cents: (amount * 100.0).round() as i64,
        currency,
        payee,
    };

    // Update memo in a scope to release the lock
    {
        let mut all = store.all.lock().unwrap();
        let account_transactions = all.get_mut(&account_id).ok_or(ApiError {
            message: "Account not found".to_string(),
            status: warp::http::StatusCode::NOT_FOUND,
        }).map_err(warp::reject::custom)?;

        let transaction = account_transactions
            .iter_mut()
            .find(|t| t.id == transaction_id)
            .ok_or(ApiError {
                message: "Transaction not found".to_string(),
                status: warp::http::StatusCode::NOT_FOUND,
            }).map_err(warp::reject::custom)?;

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