use crate::profile::Profiler;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use prometheus_client::encoding::text;
use prometheus_client::registry::Registry;
use std::sync::Arc;

const OCTET_STREAM: &str = "application/octet-stream";
const METRICS_TEXT: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

#[derive(Debug)]
pub struct RequestState {
    pub registry: Registry,
    pub profiler: Profiler,
}

pub async fn text_metrics_handler(State(state): State<Arc<RequestState>>) -> impl IntoResponse {
    let mut buf = String::new();
    let mut headers = HeaderMap::new();

    match text::encode(&mut buf, &state.registry) {
        Ok(_) => {
            tracing::debug!(message = "encoded prometheus metrics to text format", bytes = buf.len());
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(METRICS_TEXT));
            (StatusCode::OK, headers, buf.into_bytes())
        }
        Err(e) => {
            tracing::error!(message = "error encoding metrics to text format", error = %e);
            (StatusCode::INTERNAL_SERVER_ERROR, headers, Vec::new())
        }
    }
}

pub async fn pprof_handler(State(state): State<Arc<RequestState>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();

    match state.profiler.proto() {
        Ok(bytes) => {
            tracing::debug!(message = "encoded profiling data to protobuf", bytes = bytes.len());
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(OCTET_STREAM));
            (StatusCode::OK, headers, bytes)
        }
        Err(e) => {
            tracing::error!(message = "error building or encoding profiling report", error = %e);
            (StatusCode::INTERNAL_SERVER_ERROR, headers, Vec::new())
        }
    }
}
