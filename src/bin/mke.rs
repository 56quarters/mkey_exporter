use clap::Parser;
use exporter::http::RequestContext;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::sync::Arc;
use std::{io, process};
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
    pattern: String,
    label_name: String,
    label_value: String,
}

fn load_config() -> Vec<Rule> {
    vec![
        Rule {
            pattern: r"^.+:([\w]+):".to_string(),
            label_name: "user".to_string(),
            label_value: "$1".to_string(),
        },
        Rule {
            pattern: "prefix1:".to_string(),
            label_name: "type".to_string(),
            label_value: "a-longer-prefix-1".to_string(),
        },
        Rule {
            pattern: "prefix2:".to_string(),
            label_name: "type".to_string(),
            label_value: "a-longer-prefix-2".to_string(),
        },
        Rule {
            pattern: r"^([\w]+):".to_string(),
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
    let f = File::open("keys.txt").unwrap();
    let b = BufReader::new(f);

    let mut registry = <Registry>::default();
    let num_rules = Gauge::<i64>::default();
    let counts = Family::<Vec<(String, String)>, Gauge<i64>>::default();
    let sizes = Family::<Vec<(String, String)>, Gauge<i64>>::default();

    registry.register("mke_num_rules", "Number of rules", num_rules.clone());
    registry.register("mke_counts", "Counts of stuff", counts.clone());
    registry.register("mke_sizes", "Sizes of stuff", sizes.clone());

    num_rules.set(cfg.len() as i64);

    let mut val = String::new();
    let mut names = HashSet::new();
    let mut labels = Vec::new();

    let mut counts_by_labels = HashMap::new();
    let mut sizes_by_labels = HashMap::new();

    for line in b.lines() {
        let l = line.unwrap();

        names.clear();
        labels.clear();

        for r in cfg.iter() {
            if names.contains(&r.label_name) {
                continue;
            }

            let pattern = Regex::new(&r.pattern).unwrap();
            if let Some(c) = pattern.captures(&l) {
                names.insert(&r.label_name);

                c.expand(&r.label_value, &mut val);
                labels.push((r.label_name.clone(), val.clone()));
                val.clear();
            }
        }

        *counts_by_labels.entry(labels.clone()).or_insert(0_i64) += 1;
        *sizes_by_labels.entry(labels.clone()).or_insert(0_i64) += 42;
    }

    for (labels, &count) in counts_by_labels.iter() {
        counts.get_or_create(labels).set(count);
    }

    for (labels, &size) in sizes_by_labels.iter() {
        sizes.get_or_create(labels).set(size);
    }

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
