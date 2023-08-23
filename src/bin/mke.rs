use clap::Parser;
use exporter::http::RequestContext;
use mtop_client::{Meta, MemcachedPool, TLSConfig};
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::{io, process};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::signal::unix;
use tokio::signal::unix::SignalKind;
use tracing::Level;

const DEFAULT_LOG_LEVEL: Level = Level::INFO;
const DEFAULT_BIND_ADDR: ([u8; 4], u16) = ([0, 0, 0, 0], 9761);

/// Export some stuff from Memcached
#[derive(Debug, Parser)]
#[clap(name = "mke", version = clap::crate_version!())]
struct MkeApplication {
    /// Logging verbosity. Allowed values are 'trace', 'debug', 'info', 'warn', and 'error'
    /// (case insensitive)
    #[clap(long, default_value_t = DEFAULT_LOG_LEVEL)]
    log_level: Level,
    /// Address to bind to. By default, mke will bind to public address since
    /// the purpose is to expose metrics to an external system (Prometheus or another
    /// agent for ingestion)
    #[clap(long, default_value_t = DEFAULT_BIND_ADDR.into())]
    bind: SocketAddr,
}

#[derive(Debug)]
struct Rule {
    pattern: Regex,
    label_name: String,
    label_value: String,
}

fn load_config() -> Vec<Rule> {
    vec![
        Rule {
            pattern: Regex::new(r"^.+:([\w]+):").unwrap(),
            label_name: "user".to_string(),
            label_value: "$1".to_string(),
        },
        Rule {
            pattern: Regex::new(r"^([\w]+):").unwrap(),
            label_name: "type".to_string(),
            label_value: "$1".to_string(),
        },
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let opts = MkeApplication::parse();
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(opts.log_level)
            .finish(),
    )
    .expect("failed to set tracing subscriber");

    let cfg = load_config();

    let mut registry = <Registry>::default();
    let num_rules = Gauge::<i64>::default();
    let counts = Family::<Vec<(String, String)>, Gauge<i64>>::default();
    let sizes = Family::<Vec<(String, String)>, Gauge<i64>>::default();

    registry.register("mke_num_rules", "Number of rules", num_rules.clone());
    registry.register("mke_counts", "Counts of stuff", counts.clone());
    registry.register("mke_sizes", "Sizes of stuff", sizes.clone());

    num_rules.set(cfg.len() as i64);


    let pool = MemcachedPool::new(Handle::current(), TLSConfig::default()).await.unwrap();

    tokio::spawn(async move {
        let mut parser = LabelParser::new(cfg);
        let mut counts_by_labels = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;
            let mut client = pool.get("localhost:11211").await.unwrap();

            for m in client.metas().await.unwrap() {
                let labels = parser.extract(&m);
                let e = counts_by_labels
                    .entry(labels)
                    .or_insert_with(LabelCounts::default);

                e.count += 1;
                e.size += m.size as i64;
            }

            for (labels, c) in counts_by_labels.iter() {
                counts.get_or_create(labels).set(c.count);
                sizes.get_or_create(labels).set(c.size);
            }

            pool.put(client).await;
            counts_by_labels.clear();
        }
    });

    let context = Arc::new(RequestContext::new(registry));
    let handler = exporter::http::text_metrics(context);
    let (sock, server) = warp::serve(handler)
        .try_bind_with_graceful_shutdown(opts.bind, async {
            // Wait for either SIGTERM or SIGINT to shutdown
            tokio::select! {
                _ = sigterm() => {}
                _ = sigint() => {}
            }
        })
        .unwrap_or_else(|e| {
            tracing::error!(message = "error binding to address", address = "", error = %e);
            process::exit(1)
        });

    tracing::info!(message = "server started", address = %sock);
    server.await;

    tracing::info!("server shutdown");
    Ok(())
}

#[derive(Debug, Default)]
struct LabelCounts {
    count: i64,
    size: i64,
}

#[derive(Debug)]
struct LabelParser {
    config: Vec<Rule>,
    value_cache: String,
    names_cache: HashSet<String>,
}

impl LabelParser {
    fn new(config: Vec<Rule>) -> Self {
        Self {
            config,
            value_cache: String::new(),
            names_cache: HashSet::new(),
        }
    }

    fn extract(&mut self, meta: &Meta) -> Vec<(String, String)> {
        self.value_cache.clear();
        self.names_cache.clear();

        let mut labels = Vec::new();

        for rule in self.config.iter() {
            if self.names_cache.contains(&rule.label_name) {
                continue;
            }

            if let Some(c) = rule.pattern.captures(&meta.key) {
                self.names_cache.insert(rule.label_name.clone());

                c.expand(&rule.label_value, &mut self.value_cache);
                labels.push((rule.label_name.clone(), self.value_cache.clone()));
                self.value_cache.clear();
            }
        }

        labels
    }
}

/// Return after the first SIGTERM signal received by this process
async fn sigterm() -> io::Result<()> {
    unix::signal(SignalKind::terminate())?.recv().await;
    Ok(())
}

/// Return after the first SIGINT signal received by this process
async fn sigint() -> io::Result<()> {
    unix::signal(SignalKind::interrupt())?.recv().await;
    Ok(())
}
