use clap::{Parser, ValueHint};
use mkey_exporter::keys::LabelParser;
use mkey_exporter::metrics::{Metrics, RequestContext};
use mtop_client::{MemcachedPool, MtopError, PoolConfig, TLSConfig};
use prometheus_client::registry::Registry;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::{io, process};
use tokio::runtime::Handle;
use tokio::signal::unix;
use tokio::signal::unix::SignalKind;
use tokio::time::Instant;
use tracing::Level;

const DEFAULT_BIND_ADDR: ([u8; 4], u16) = ([0, 0, 0, 0], 9761);
const DEFAULT_REFRESH_SECS: u64 = 180;
const DEFAULT_LOG_LEVEL: Level = Level::INFO;
const DEFAULT_HOST: &str = "localhost:11211";

/// Export metadata about memcached entries based on rules applied to their keys.
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
    #[arg(long, default_value_t = DEFAULT_REFRESH_SECS)]
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

    /// Path to configuration file providing key parsing rules.
    #[arg(required = true, value_hint = ValueHint::FilePath)]
    config: PathBuf,
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

    let cfg = mkey_exporter::config::from_path(&opts.config).unwrap_or_else(|e| {
        tracing::error!(message = "unable to parse rule configuration", path = ?opts.config, err = %e);
        process::exit(1);
    });

    let pool = MemcachedPool::new(
        Handle::current(),
        PoolConfig {
            tls: TLSConfig {
                enabled: opts.tls_enabled,
                ca_path: opts.tls_ca,
                cert_path: opts.tls_cert,
                key_path: opts.tls_key,
                server_name: opts.tls_server_name,
            },
            ..Default::default()
        },
    )
    .await
    .unwrap_or_else(|e| {
        tracing::error!(message = "unable to initialize memcached client", host = %opts.host, err = %e);
        process::exit(1);
    });

    connect(&opts.host, &pool).await.unwrap_or_else(|e| {
        tracing::error!(message = "unable to connect to memcached host", host = %opts.host, err = %e);
        process::exit(1);
    });

    let mut registry = Registry::default();
    let metrics = Metrics::new(&mut registry);

    tokio::spawn(async move {
        let mut parser = LabelParser::new(&cfg);
        let mut interval = tokio::time::interval(Duration::from_secs(opts.refresh_secs));
        let mut to_remove = HashSet::new();

        loop {
            let start = interval.tick().await;

            let mut client = match pool.get(&opts.host).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(message = "failed to connect to server", host = %opts.host, err = %e);
                    metrics.incr_failure();
                    continue;
                }
            };

            let metas = match client.metas().await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(message = "failed to fetch key metas", host = %opts.host, err = %e);
                    metrics.incr_failure();
                    continue;
                }
            };

            let mut counts_by_labels = HashMap::new();
            let num_keys = metas.len();

            for m in metas.iter() {
                let labels = parser.extract(m);
                to_remove.remove(&labels);

                let e = counts_by_labels.entry(labels).or_insert_with(LabelCounts::default);

                e.count += 1;
                e.size += m.size as i64;
            }

            // At the end of every update loop, we add all the unique label sets we found to
            // the "to remove" set. At the beginning of the next update loop, we remove any of
            // the labels that are generated from the new meta objects from the "to remove" set.
            // At this point in the loop we are left with a set of labels that existed the last
            // iteration but no longer do and are thus safe to remove from our metric registry.
            metrics.cleanup_keys(&to_remove);
            to_remove.clear();

            let num_unique_labels = counts_by_labels.len();
            for (labels, c) in counts_by_labels {
                metrics.update_key(&labels, c.count, c.size);
                to_remove.insert(labels);
            }

            // Try to reduce memory usage down from the high-water mark.
            to_remove.shrink_to_fit();

            let time_taken = Instant::now().duration_since(start);
            tracing::info!(
                message = "updated metrics for memcached keys",
                rule_group = cfg.name,
                num_keys = num_keys,
                num_unique_labels = num_unique_labels,
                time_taken = ?time_taken,
            );
            metrics.incr_success(time_taken);
            pool.put(client).await;
        }
    });

    let context = Arc::new(RequestContext::new(registry));
    let filter = mkey_exporter::metrics::text_metrics_filter(context);
    let (sock, server) = warp::serve(filter)
        .try_bind_with_graceful_shutdown(opts.bind, async {
            // Wait for either SIGTERM or SIGINT to shutdown
            tokio::select! {
                _ = sigterm() => {}
                _ = sigint() => {}
            }
        })
        .unwrap_or_else(|e| {
            tracing::error!(message = "error binding to address", address = %opts.bind, err = %e);
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

async fn connect(host: &str, pool: &MemcachedPool) -> Result<(), MtopError> {
    let client = pool.get(host).await?;
    pool.put(client).await;
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
