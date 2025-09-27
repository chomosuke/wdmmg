mod error;
mod handlers;
mod types;
mod utils;

use error::handle_rejection;
use handlers::*;
use types::TransactionStore;
use utils::with_store;
use warp::Filter;

#[tokio::main]
async fn main() {
    let store = TransactionStore::new();

    // Load existing data from files
    if let Err(e) = store.load_from_files().await {
        eprintln!("Warning: Failed to load existing data: {}", e);
    }

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "PUT"]);

    // GET /transactions/current - Get current transactions
    let get_current_transactions = warp::path!("transactions" / "current")
        .and(warp::get())
        .and(with_store(store.clone()))
        .and_then(get_current_transactions_handler);

    // GET /transactions/all - Get all historical transactions
    let get_all_transactions = warp::path!("transactions" / "all")
        .and(warp::get())
        .and(with_store(store.clone()))
        .and_then(get_all_transactions_handler);

    // POST /transactions - Create a new transaction
    let create_transaction = warp::path("transactions")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_store(store.clone()))
        .and_then(create_transaction_handler);

    // POST /transactions/bulk/:account_id - Upload CSV for bulk import
    let bulk_import = warp::path!("transactions" / "bulk" / String)
        .and(warp::post())
        .and(warp::body::bytes())
        .and(with_store(store.clone()))
        .and_then(bulk_import_handler);

    // PUT /transactions/:account_id/memo - Update transaction memo
    let update_memo = warp::path!("transactions" / String / "memo")
        .and(warp::put())
        .and(warp::body::json())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(with_store(store.clone()))
        .and_then(update_memo_handler);

    let routes = get_current_transactions
        .or(get_all_transactions)
        .or(create_transaction)
        .or(bulk_import)
        .or(update_memo)
        .with(cors)
        .recover(handle_rejection);

    println!("Server running on http://localhost:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
