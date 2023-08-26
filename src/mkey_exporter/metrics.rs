use prometheus_client::encoding::text;
use prometheus_client::registry::Registry;
use std::sync::Arc;
use warp::http::header::CONTENT_TYPE;
use warp::http::{HeaderValue, StatusCode};
use warp::reply::Response;
use warp::{Filter, Rejection, Reply};

const TEXT_FORMAT: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

/// Global stated shared between all HTTP requests via Arc.
pub struct RequestContext {
    registry: Registry,
}

impl RequestContext {
    pub fn new(registry: Registry) -> Self {
        RequestContext { registry }
    }
}

/// Create a warp Filter implementation that renders Prometheus metrics from
/// a registry in the text exposition format at the path `/metrics` for `GET`
/// requests. If an error is encountered, an HTTP 500 will be returned and the
/// error will be logged.
pub fn text_metrics_filter(
    context: Arc<RequestContext>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("metrics")
        .and(warp::filters::method::get())
        .map(move || {
            let context = context.clone();
            let mut buf = String::new();

            match text::encode(&mut buf, &context.registry) {
                Ok(_) => {
                    tracing::debug!(message = "encoded prometheus metrics to text format",);
                    let mut res = Response::new(buf.into());
                    res.headers_mut()
                        .insert(CONTENT_TYPE, HeaderValue::from_static(TEXT_FORMAT));
                    res
                }
                Err(e) => {
                    tracing::error!(message = "error encoding metrics to text format", error = %e);
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        })
}
