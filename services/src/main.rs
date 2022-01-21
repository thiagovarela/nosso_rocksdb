use axum::{
    body::Bytes,
    error_handling::HandleErrorLayer,
    extract::{ContentLengthLimit, Extension, Path},
    handler::Handler,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    AddExtensionLayer, Router,
};
use std::{borrow::Cow, sync::Arc, time::Duration};
use tower::{BoxError, ServiceBuilder};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
mod protos;

use crate::protos::nosso::users::v1::User;
use prost::Message;

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

type Database = Arc<rocksdb::DB>;

fn open_database(path: &str) -> Database {
    let prefix_extractor = rocksdb::SliceTransform::create_fixed_prefix(3);

    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.set_prefix_extractor(prefix_extractor);

    let db = rocksdb::DB::open_default(path).unwrap();
    Arc::new(db)
}

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "services=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    // Build our application by composing routes
    let app = app("/tmp/db");

    // Run our app with hyper
    let address = "[::0]:50051".parse().unwrap();
    tracing::debug!("listening on {}", address);
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn app(database_path: &str) -> Router {
    let app_state = open_database(database_path);
    Router::new()
        .route(
            "/:key",
            // Add compression to `kv_get`
            get(kv_get.layer(CompressionLayer::new()))
                // But don't compress `kv_set`
                .post(kv_set),
        )
        // Nest our admin routes under `/admin`
        .nest("/admin", admin_routes())
        // Add middleware to all routes
        .layer(
            ServiceBuilder::new()
                // Handle errors from middleware
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                .concurrency_limit(1024)
                .timeout(Duration::from_secs(10))
                .layer(TraceLayer::new_for_http())
                .layer(AddExtensionLayer::new(app_state))
                .into_inner(),
        )
}

async fn kv_get(
    Path(key): Path<String>,
    Extension(db): Extension<Database>,
) -> Result<Bytes, StatusCode> {
    tracing::debug!("Getting key {}", key);
    match db.get(&key) {
        Ok(Some(v)) => Ok(Bytes::from(v)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

async fn kv_set(
    Path(key): Path<String>,
    Extension(db): Extension<Database>,
    ContentLengthLimit(bytes): ContentLengthLimit<Bytes, { 1024 * 5_000 }>, // ~5mb
) {
    tracing::debug!("Setting key {}", key);
    db.put(&key, bytes).unwrap();
    // state.write().unwrap().db.insert(key, bytes);
}

fn admin_routes() -> Router {
    async fn install_get(Extension(db): Extension<Database>) -> StatusCode {
        match db.get("install-token-key") {
            Ok(Some(_)) => StatusCode::GONE,
            Ok(None) => StatusCode::OK,
            _ => StatusCode::OK,
        }
    }

    async fn install_post(
        body: Bytes,
        Extension(db): Extension<Database>,
    ) -> Result<Bytes, StatusCode> {
        let installed = match db.get("install-token-key") {
            Ok(Some(_)) => StatusCode::GONE,
            Ok(None) => StatusCode::OK,
            _ => StatusCode::OK,
        };
        match installed {
            StatusCode::OK => {
                let mut user = User::decode(body).unwrap();
                user.id = uuid::Uuid::new_v4().to_string();
                tracing::debug!("{:?}", user);
                // do other stuff
                let encoded = Bytes::from(user.encode_to_vec());
                let key = format!("users_{}", user.id);
                db.put(key, &encoded).unwrap();
                db.put("install-token-key", "installed").unwrap();
                Ok(encoded)
            }
            _ => Err(installed),
        }
    }

    Router::new().route("/install", get(install_get).post(install_post))
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {}", error)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn it_can_reach_install() {
        let app = app("/tmp/hello_db");

        // `Router` implements `tower::Service<Request<Body>>` so we can
        // call it like any tower service, no need to run an HTTP server.
        let response = app
            .oneshot(Request::builder().uri("/admin/install").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}