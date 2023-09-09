use prometheus_client::encoding::text;
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue, LabelValueEncoder};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::{Registry, Unit};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use warp::http::header::CONTENT_TYPE;
use warp::http::{HeaderValue, StatusCode};
use warp::reply::Response;
use warp::{Filter, Rejection, Reply};

const TEXT_FORMAT: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";
const DEFAULT_BUCKETS: &[f64] = &[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0];
const RESULT_SUCCESS: UpdateResultLabels = UpdateResultLabels {
    result: UpdateResult::Success,
};
const RESULT_FAILURE: UpdateResultLabels = UpdateResultLabels {
    result: UpdateResult::Failure,
};

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
pub fn http_text_metrics(context: Arc<RequestContext>) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("metrics").and(warp::filters::method::get()).map(move || {
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

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct UpdateResultLabels {
    result: UpdateResult,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum UpdateResult {
    Success,
    Failure,
}

impl EncodeLabelValue for UpdateResult {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), std::fmt::Error> {
        match self {
            UpdateResult::Success => EncodeLabelValue::encode(&"success", encoder),
            UpdateResult::Failure => EncodeLabelValue::encode(&"failure", encoder),
        }
    }
}

#[derive(Debug)]
pub struct Metrics {
    updates: Family<UpdateResultLabels, Counter>,
    duration: Histogram,
    counts: Family<Vec<(String, String)>, Gauge<i64>>,
    sizes: Family<Vec<(String, String)>, Gauge<i64>>,
}

impl Metrics {
    pub fn new(reg: &mut Registry) -> Self {
        let updates = Family::<UpdateResultLabels, Counter>::default();
        let duration = Histogram::new(DEFAULT_BUCKETS.iter().copied());
        let counts = Family::<Vec<(String, String)>, Gauge<i64>>::default();
        let sizes = Family::<Vec<(String, String)>, Gauge<i64>>::default();

        reg.register(
            "mkey_updates",
            "How many update loops have been run by the result",
            updates.clone(),
        );
        reg.register_with_unit(
            "mkey_updates_duration",
            "How long update loops take in seconds",
            Unit::Seconds,
            duration.clone(),
        );
        reg.register(
            "mkey_memcached_counts",
            "Counts of keys matching the supplied configuration",
            counts.clone(),
        );
        reg.register(
            "mkey_memcached_sizes",
            "Total size of all keys matching the supplied configuration",
            sizes.clone(),
        );

        Self {
            updates,
            duration,
            counts,
            sizes,
        }
    }

    pub fn incr_failure(&self) {
        self.updates.get_or_create(&RESULT_FAILURE).inc();
    }

    pub fn incr_success(&self, duration: Duration) {
        self.duration.observe(duration.as_secs_f64());
        self.updates.get_or_create(&RESULT_SUCCESS).inc();
    }

    pub fn update_key(&self, labels: &Vec<(String, String)>, count: i64, size: i64) {
        self.counts.get_or_create(labels).set(count);
        self.sizes.get_or_create(labels).set(size);
    }

    pub fn cleanup_keys(&self, labels_to_remove: &HashSet<Vec<(String, String)>>) {
        let mut counts_removed = 0;
        let mut sizes_removed = 0;

        for e in labels_to_remove.iter() {
            if self.counts.remove(e) {
                counts_removed += 1;
            }

            if self.sizes.remove(e) {
                sizes_removed += 1;
            }
        }

        tracing::debug!(
            message = "cleaned up unused label sets",
            counts_series_cleaned = counts_removed,
            sizes_series_cleaned = sizes_removed
        );
    }
}
