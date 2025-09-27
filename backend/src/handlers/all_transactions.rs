use crate::types::TransactionStore;
use warp;

pub async fn get_all_transactions_handler(
    store: TransactionStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    let all = store.all.lock().unwrap();
    let mut all_transactions = Vec::new();
    for transactions in all.values() {
        all_transactions.extend(transactions.iter().cloned());
    }
    Ok(warp::reply::json(&all_transactions))
}