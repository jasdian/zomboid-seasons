pub mod admin;
pub mod internal;
pub mod models;
pub mod public;

pub fn build_api_router() -> axum::Router<crate::AppState> {
    axum::Router::new()
        .merge(public::router())
        .merge(admin::router())
}

pub fn build_internal_router() -> axum::Router<crate::AppState> {
    internal::router()
}
