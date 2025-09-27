use crate::error::ApiError;
use crate::types::*;
use std::collections::HashMap;
use warp;

pub async fn create_transaction_handler(
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
        let account_transactions = current
            .entry(request.account_id.clone())
            .or_insert_with(HashMap::new);

        if account_transactions.contains_key(&transaction_id) {
            return Err(warp::reject::custom(ApiError {
                message: "Transaction already exists".to_string(),
                status: warp::http::StatusCode::CONFLICT,
            }));
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