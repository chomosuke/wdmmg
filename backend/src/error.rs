use serde_json;
use warp;

#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub status: warp::http::StatusCode,
}

impl warp::reject::Reject for ApiError {}

pub async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    if let Some(api_error) = err.find::<ApiError>() {
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": api_error.message
            })),
            api_error.status,
        ))
    } else if err.is_not_found() {
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "Not found"
            })),
            warp::http::StatusCode::NOT_FOUND,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "Internal server error"
            })),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}