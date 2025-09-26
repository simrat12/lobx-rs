use tracing_subscriber::EnvFilter;

#[cfg(feature = "metrics-exporter")]
use metrics_exporter_prometheus::PrometheusBuilder;

pub fn init_tracing(default_filter: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
}

#[cfg(feature = "metrics-exporter")]
pub fn init_metrics() {
    use metrics_exporter_prometheus::PrometheusBuilder;

    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .expect("prometheus exporter install");

    // prove the exporter is alive
    println!("Prometheus exporter listening on http://0.0.0.0:9000/metrics");
    metrics::gauge!("lobx_up").set(1.0);
}


#[cfg(not(feature = "metrics-exporter"))]
pub fn init_metrics() { /* no-op */ }
