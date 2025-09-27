use crate::store::TransactionStore;
use crate::types::CreateTransactionRequest;
use warp;

pub async fn create_transaction_handler(
    request: CreateTransactionRequest,
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let current_transaction = store.create_transaction(request).await.map_err(warp::reject::custom)?;

    Ok(warp::reply::with_status(
        warp::reply::json(&current_transaction),
        warp::http::StatusCode::CREATED,
    ))
}