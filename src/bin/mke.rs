use exporter::http::RequestContext;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::sync::Arc;
use std::{io, process};
use tokio::signal::unix;
use tokio::signal::unix::SignalKind;

const DEFAULT_BIND_ADDR: ([u8; 4], u16) = ([0, 0, 0, 0], 8000);

#[derive(Debug)]
struct Rule {
    pattern: String,
    label_name: String,
    label_value: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cfg = vec![
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
    ];

    let f = File::open("keys.txt").unwrap();
    let b = BufReader::new(f);

    let mut val = String::new();
    let mut series = Vec::new();
    let mut labels = HashMap::new();
    let mut label_vec = Vec::new();

    let unique: BTreeSet<String> = cfg.iter().map(|c| c.label_name.clone()).collect();

    let mut registry = <Registry>::default();
    let num_rules = Gauge::<i64>::default();
    let counts = Family::<Vec<(String, String)>, Gauge<i64>>::default();

    registry.register(
        "mke_num_rules",
        "Number of defined label rules",
        num_rules.clone(),
    );
    registry.register("mke_counts", "Counts of stuff", counts.clone());

    num_rules.set(cfg.len() as i64);

    for line in b.lines() {
        let l = line.unwrap();
        labels.clear();
        label_vec.clear();

        for r in cfg.iter() {
            if labels.contains_key(&r.label_name) {
                continue;
            }

            let pattern = Regex::new(&r.pattern).unwrap();
            if let Some(c) = pattern.captures(&l) {
                c.expand(&r.label_value, &mut val);
                labels.insert(&r.label_name, val.clone());
                label_vec.push((r.label_name.clone(), val.clone()));
                val.clear();
            }
        }

        counts.get_or_create(&label_vec).inc();

        series.push(labels.clone());
    }

    println!("unique: {:?}", unique);

    for s in series {
        println!("series: {:?}", s);
    }

    let addr: SocketAddr = DEFAULT_BIND_ADDR.into();

    let context = Arc::new(RequestContext::new(registry));
    let handler = exporter::http::text_metrics(context);
    let (sock, server) = warp::serve(handler)
        .try_bind_with_graceful_shutdown(addr, async {
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
