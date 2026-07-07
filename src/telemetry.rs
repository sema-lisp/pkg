//! Production observability: OpenTelemetry traces + Prometheus metrics.
//!
//! Everything here is **runtime-gated** — compiled into the binary but a no-op
//! until configured by environment variables, so it is safe to ship enabled.
//!
//! ## Traces (`OTEL_TRACES_EXPORTER`)
//! - `none` (default) — no span export; plain `tracing` logs only.
//! - `file` — one JSON span per line to `OTEL_TRACE_FILE` (default
//!   `traces.jsonl`). Zero infrastructure; grep it or feed it to a viewer.
//! - `otlp` — OTLP/gRPC to `OTEL_EXPORTER_OTLP_ENDPOINT`
//!   (default `http://localhost:4317`) for Jaeger, Grafana Tempo, or an
//!   OpenTelemetry Collector.
//!
//! Common OTel env vars are honoured: `OTEL_SERVICE_NAME` (default `sema-pkg`),
//! `OTEL_TRACES_SAMPLER_ARG` (head sampling ratio 0.0–1.0, default 1.0).
//! `RUST_LOG` filters logs; `OTEL_LOG` filters span *capture* independently
//! (default `info,sema_pkg=trace,tower_http=debug`), so quiet logs and rich
//! traces coexist.
//!
//! ## Metrics (`METRICS_ENABLED=true`)
//! Installs a Prometheus recorder and serves `/metrics` for scraping:
//! - per-request RED metrics (`http_requests_total`,
//!   `http_request_duration_seconds`, `http_requests_in_flight`), labelled by
//!   the matched route template to keep cardinality bounded;
//! - process metrics (memory, CPU, open FDs, threads);
//! - application gauges (`sema_packages_total`, `sema_users_total`, …).
//!
//! To carry these into an OTel/OTLP pipeline, point an OpenTelemetry Collector's
//! `prometheus` receiver at `/metrics` (see `docker-compose.observability.yml`).

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use opentelemetry_sdk::Resource;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::MetricsRender;

/// Flushes and shuts the tracer down when dropped. Keep alive for the process
/// lifetime (a `let _guard` in `main`).
pub struct Guard(Option<SdkTracerProvider>);

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = p.shutdown();
        }
    }
}

/// Install the tracing subscriber (fmt logs always; OpenTelemetry span export
/// when `OTEL_TRACES_EXPORTER` selects one). Call once, early in `main`.
pub fn init() -> Guard {
    let service = env("OTEL_SERVICE_NAME").unwrap_or_else(|| "sema-pkg".to_string());
    let provider = match env("OTEL_TRACES_EXPORTER").as_deref() {
        Some("file") => Some(file_provider(&service)),
        Some("otlp") => Some(otlp_provider(&service)),
        _ => None,
    };

    // Log verbosity (RUST_LOG) and trace capture (OTEL_LOG) are filtered
    // independently: an operator can run `RUST_LOG=warn` for quiet logs while
    // still capturing the request/DAL span tree for export.
    let log_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().with_filter(log_filter);

    // `Option<Layer>` is itself a Layer (None = disabled), so the chain works
    // whether or not tracing is exported.
    let otel_layer = provider.as_ref().map(|p| {
        let trace_filter = EnvFilter::try_from_env("OTEL_LOG")
            .unwrap_or_else(|_| EnvFilter::new("info,sema_pkg=trace,tower_http=debug"));
        tracing_opentelemetry::layer()
            .with_tracer(p.tracer("sema-pkg"))
            .with_filter(trace_filter)
    });

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    match &provider {
        Some(_) => eprintln!(
            "OpenTelemetry traces: {} exporter",
            env("OTEL_TRACES_EXPORTER").unwrap_or_default()
        ),
        None => { /* traces disabled — logs only */ }
    }
    Guard(provider)
}

fn resource(service: &str) -> Resource {
    Resource::builder()
        .with_service_name(service.to_string())
        .build()
}

/// Head sampler from `OTEL_TRACES_SAMPLER_ARG` (ratio), parent-based so a
/// sampled parent keeps its children.
fn sampler() -> Sampler {
    let ratio = env("OTEL_TRACES_SAMPLER_ARG")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(ratio)))
}

fn file_provider(service: &str) -> SdkTracerProvider {
    let path = env("OTEL_TRACE_FILE").unwrap_or_else(|| "traces.jsonl".to_string());
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| panic!("failed to open OTEL_TRACE_FILE '{path}': {e}"));
    eprintln!("OpenTelemetry traces → {path}");
    SdkTracerProvider::builder()
        .with_resource(resource(service))
        .with_sampler(sampler())
        .with_simple_exporter(file_exporter::FileExporter::new(file))
        .build()
}

fn otlp_provider(service: &str) -> SdkTracerProvider {
    use opentelemetry_otlp::WithExportConfig as _;
    let mut builder = opentelemetry_otlp::SpanExporter::builder().with_tonic();
    if let Some(endpoint) = env("OTEL_EXPORTER_OTLP_ENDPOINT") {
        builder = builder.with_endpoint(endpoint.clone());
        eprintln!("OpenTelemetry traces → OTLP {endpoint}");
    }
    let exporter = builder.build().expect("failed to build OTLP span exporter");
    SdkTracerProvider::builder()
        .with_resource(resource(service))
        .with_sampler(sampler())
        .with_batch_exporter(exporter)
        .build()
}

/// Install a Prometheus recorder and return a `/metrics` render closure, or
/// `None` when metrics are disabled (`METRICS_ENABLED` unset/false).
pub fn init_metrics() -> Option<MetricsRender> {
    if env("METRICS_ENABLED").as_deref() != Some("true") {
        return None;
    }
    use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
    // Latency buckets tuned for a web service (1ms … 10s).
    let buckets = [
        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_request_duration_seconds".to_string()),
            &buckets,
        )
        .expect("valid histogram buckets")
        .install_recorder()
        .expect("install Prometheus recorder");
    eprintln!("Prometheus metrics: /metrics enabled");
    Some(Arc::new(move || handle.render()))
}

/// Spawn a background task publishing process and application gauges to the
/// metrics recorder. Call once, after the DB is connected and only when metrics
/// are enabled. Emits:
/// - process metrics (`process_resident_memory_bytes`, `process_cpu_seconds_total`,
///   `process_open_fds`, `process_threads`, …) via [`metrics_process`];
/// - application gauges refreshed every 15s: `sema_packages_total`,
///   `sema_users_total`, `sema_users_banned`, `sema_reports_open`,
///   `sema_downloads_30d`;
/// - a static `sema_build_info{version}` gauge.
pub fn spawn_collectors(db: crate::db::Db) {
    metrics::gauge!("sema_build_info", "version" => env!("CARGO_PKG_VERSION")).set(1.0);

    let process = metrics_process::Collector::default();
    process.describe();

    tokio::spawn(async move {
        loop {
            process.collect();
            let s = crate::dal::admin::stats(&db).await;
            metrics::gauge!("sema_packages_total").set(s.total_packages as f64);
            metrics::gauge!("sema_users_total").set(s.total_users as f64);
            metrics::gauge!("sema_users_banned").set(s.banned_users as f64);
            metrics::gauge!("sema_reports_open").set(s.open_reports as f64);
            metrics::gauge!("sema_downloads_30d").set(s.total_downloads as f64);
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}

/// Axum middleware recording RED metrics per request, keyed by the matched route
/// template so package-name paths don't explode label cardinality. A no-op when
/// no metrics recorder is installed.
pub async fn track_metrics(req: Request, next: Next) -> Response {
    let method = req.method().as_str().to_string();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());

    metrics::gauge!("http_requests_in_flight").increment(1.0);
    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();
    metrics::gauge!("http_requests_in_flight").decrement(1.0);

    let status = response.status().as_u16().to_string();
    metrics::counter!(
        "http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status,
    )
    .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "path" => path,
    )
    .record(elapsed);

    response
}

/// An OpenTelemetry span exporter that appends each span as one JSON line.
mod file_exporter {
    use std::io::Write;
    use std::sync::Mutex;
    use std::time::UNIX_EPOCH;

    use opentelemetry_sdk::error::OTelSdkResult;
    use opentelemetry_sdk::trace::{SpanData, SpanExporter};

    #[derive(Debug)]
    pub struct FileExporter {
        out: Mutex<std::io::BufWriter<std::fs::File>>,
    }

    impl FileExporter {
        pub fn new(file: std::fs::File) -> Self {
            Self {
                out: Mutex::new(std::io::BufWriter::new(file)),
            }
        }
    }

    impl SpanExporter for FileExporter {
        async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
            if let Ok(mut w) = self.out.lock() {
                for s in batch {
                    let start_ns = s
                        .start_time
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    let duration_ms = s
                        .end_time
                        .duration_since(s.start_time)
                        .map(|d| d.as_secs_f64() * 1000.0)
                        .unwrap_or(0.0);
                    let attributes: serde_json::Map<String, serde_json::Value> = s
                        .attributes
                        .iter()
                        .map(|kv| {
                            (
                                kv.key.as_str().to_string(),
                                serde_json::Value::String(kv.value.to_string()),
                            )
                        })
                        .collect();
                    let line = serde_json::json!({
                        "trace_id": s.span_context.trace_id().to_string(),
                        "span_id": s.span_context.span_id().to_string(),
                        "parent_span_id": s.parent_span_id.to_string(),
                        "name": s.name,
                        "kind": format!("{:?}", s.span_kind),
                        "status": format!("{:?}", s.status),
                        "start_unix_ns": start_ns.to_string(),
                        "duration_ms": duration_ms,
                        "attributes": attributes,
                    });
                    let _ = writeln!(w, "{line}");
                }
                let _ = w.flush();
            }
            Ok(())
        }

        fn shutdown(&self) -> OTelSdkResult {
            if let Ok(mut w) = self.out.lock() {
                let _ = w.flush();
            }
            Ok(())
        }
    }
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}
