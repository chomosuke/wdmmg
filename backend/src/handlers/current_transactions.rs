use crate::types::TransactionStore;
use warp;

pub async fn get_current_transactions_handler(
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let current = store.current.lock().unwrap();
    let mut all_transactions = Vec::new();
    for transactions in current.values() {
        all_transactions.extend(transactions.values().cloned());
    }
    Ok(warp::reply::json(&all_transactions))
}