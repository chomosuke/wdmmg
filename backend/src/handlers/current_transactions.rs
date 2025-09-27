use crate::store::TransactionStore;
use warp;

pub async fn get_current_transactions_handler(
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let transactions = store.get_current_transactions();
    Ok(warp::reply::json(&transactions))
}