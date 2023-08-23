use clap::{Parser, ValueHint};
use mkey_exporter::config::Rule;
use mkey_exporter::http::RequestContext;
use mkey_exporter::keys::LabelParser;
use mtop_client::{MemcachedPool, TLSConfig};
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use regex::Regex;
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::{io, process};
use tokio::runtime::Handle;
use tokio::signal::unix;
use tokio::signal::unix::SignalKind;
use tracing::Level;

const DEFAULT_BIND_ADDR: ([u8; 4], u16) = ([0, 0, 0, 0], 9761);
const DEFAULT_REFERSH_SECS: u64 = 60;
const DEFAULT_LOG_LEVEL: Level = Level::INFO;
const DEFAULT_HOST: &str = "localhost:11211";

/// Export some stuff from Memcached
#[derive(Debug, Parser)]
#[clap(name = "mkey_exporter", version = clap::crate_version!())]
struct MkeyExporterApplication {
    /// Logging verbosity. Allowed values are 'trace', 'debug', 'info', 'warn', and 'error'
    /// (case insensitive)
    #[arg(long, default_value_t = DEFAULT_LOG_LEVEL)]
    log_level: Level,

    /// Address to bind to. By default, the server will bind to public address since
    /// the purpose is to expose metrics to an external system (Prometheus or another
    /// agent for ingestion)
    #[arg(long, default_value_t = DEFAULT_BIND_ADDR.into())]
    bind: SocketAddr,

    /// Memcached host to connect to in the form 'hostname:port'.
    #[arg(long, default_value_t = DEFAULT_HOST.to_owned(), value_hint = ValueHint::Hostname)]
    host: String,

    /// Fetch cache keys from the Memcached server at this interval, in seconds
    #[arg(long, default_value_t = DEFAULT_REFERSH_SECS)]
    refresh_secs: u64,

    /// Enable TLS connections to the Memcached server.
    #[arg(long)]
    tls_enabled: bool,

    /// Optional certificate authority to use for validating the server certificate instead of
    /// the default root certificates.
    #[arg(long, value_hint = ValueHint::FilePath)]
    tls_ca: Option<PathBuf>,

    /// Optional server name to use for validating the server certificate. If not set, the
    /// hostname of the server is used for checking that the certificate matches the server.
    #[arg(long)]
    tls_server_name: Option<String>,

    /// Optional client certificate to use to authenticate with the Memcached server. Note that
    /// this may or may not be required based on how the Memcached server is configured.
    #[arg(long, requires = "tls_key", value_hint = ValueHint::FilePath)]
    tls_cert: Option<PathBuf>,

    /// Optional client key to use to authenticate with the Memcached server. Note that this may
    /// or may not be required based on how the Memcached server is configured.
    #[arg(long, requires = "tls_cert", value_hint = ValueHint::FilePath)]
    tls_key: Option<PathBuf>,
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
    let opts = MkeyExporterApplication::parse();
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

    registry.register("mkey_num_rules", "Number of rules", num_rules.clone());
    registry.register("mkey_counts", "Counts of stuff", counts.clone());
    registry.register("mkey_sizes", "Sizes of stuff", sizes.clone());

    num_rules.set(cfg.len() as i64);

    let pool = MemcachedPool::new(
        Handle::current(),
        TLSConfig {
            enabled: opts.tls_enabled,
            ca_path: opts.tls_ca,
            cert_path: opts.tls_cert,
            key_path: opts.tls_key,
            server_name: opts.tls_server_name,
        },
    )
    .await
    .unwrap_or_else(|e| {
        tracing::error!(message = "unable to initialize memcached client", host = %opts.host, error = %e);
        process::exit(1);
    });

    tokio::spawn(async move {
        let mut parser = LabelParser::new(&cfg);
        let mut counts_by_labels = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(opts.refresh_secs));

        loop {
            interval.tick().await;

            let mut client = match pool.get(&opts.host).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(message = "failed to connect to server", host = %opts.host, err = %e);
                    continue;
                }
            };

            let metas = match client.metas().await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(message = "failed to fetch key metas", host = %opts.host, err = %e);
                    continue;
                }
            };

            for m in metas {
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
    let handler = mkey_exporter::http::text_metrics(context);
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
