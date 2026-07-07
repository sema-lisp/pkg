//! OpenTelemetry tracing of request + DB paths, exported to a JSONL file
//! (feature `otel`). One line per span:
//!
//! ```json
//! {"trace_id":"…","span_id":"…","parent_span_id":"…","name":"admin.list_users",
//!  "duration_ms":10804.2,"attributes":{…}}
//! ```
//!
//! A file exporter (rather than OTLP) keeps this zero-infrastructure: build with
//! `--features otel`, run, and read/grep `traces.jsonl`. Reconstruct a trace tree
//! by grouping on `trace_id` and linking `parent_span_id → span_id`.
//!
//! The exporter writes to `OTEL_TRACE_FILE` (default `traces.jsonl`).

use std::io::Write;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{SdkTracerProvider, SpanData, SpanExporter};
use opentelemetry_sdk::Resource;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Flushes and shuts the tracer down when dropped — keep it alive for the
/// lifetime of the process (e.g. a `let _guard` in `main`).
pub struct Guard(SdkTracerProvider);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

/// Install the tracing subscriber (fmt + OpenTelemetry file export) and return a
/// flush guard. Call once, early in `main`.
pub fn init() -> Guard {
    let path = std::env::var("OTEL_TRACE_FILE").unwrap_or_else(|_| "traces.jsonl".into());
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| panic!("failed to open OTEL_TRACE_FILE '{path}': {e}"));
    let exporter = FileExporter {
        out: Mutex::new(std::io::BufWriter::new(file)),
    };

    let provider = SdkTracerProvider::builder()
        .with_resource(Resource::builder().with_service_name("sema-pkg").build())
        .with_simple_exporter(exporter)
        .build();
    let tracer = provider.tracer("sema-pkg");

    // Default to tracing our own crate finely plus tower-http's per-request span
    // (so a trace tree is request → handler → query), while keeping other deps
    // quiet. Override with RUST_LOG.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sema_pkg=trace,tower_http=debug"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    eprintln!("OpenTelemetry tracing enabled → {path}");
    Guard(provider)
}

/// A minimal OpenTelemetry span exporter that appends each span as one JSON line.
#[derive(Debug)]
struct FileExporter {
    out: Mutex<std::io::BufWriter<std::fs::File>>,
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
