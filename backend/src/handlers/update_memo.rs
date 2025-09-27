use crate::store::TransactionStore;
use crate::types::{TransactionId, UpdateMemoRequest};
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

    store.update_transaction_memo(account_id, transaction_id, memo_request.memo).await.map_err(warp::reject::custom)?;

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({"message": "Memo updated successfully"})),
        warp::http::StatusCode::OK,
    ))
}