# LOBX Order Engine Monitoring Setup

This guide will help you set up comprehensive monitoring for the LOBX order engine using Prometheus and Grafana to visualize latency metrics.

## Overview

The monitoring stack includes:
- **Prometheus**: Metrics collection and storage
- **Grafana**: Metrics visualization and dashboards
- **LOBX Application**: Exposes metrics on port 9000

## Available Metrics

The LOBX order engine exposes the following latency metrics:

- `lobx_submit_latency_ns`: Overall order submission latency
- `lobx_limit_order_latency_ns`: Limit order execution latency
- `lobx_market_order_latency_ns`: Market order execution latency
- `lobx_cancel_order_latency_ns`: Order cancellation latency
- `lobx_order_matching_latency_ns`: Order matching operation latency
- `lobx_order_resting_latency_ns`: Order resting operation latency
- `lobx_submit_total`: Total order submissions counter

## Quick Start

### 1. Start the Monitoring Stack

```bash
# Start Prometheus and Grafana
docker-compose -f docker-compose.monitoring.yml up -d

# Check that services are running
docker-compose -f docker-compose.monitoring.yml ps
```

### 2. Start the LOBX Application

```bash
# Build and run with metrics enabled
cargo run --features metrics-exporter
```

### 3. Access the Services

- **Grafana Dashboard**: http://localhost:3000
  - Username: `admin`
  - Password: `admin`
- **Prometheus**: http://localhost:9090
- **LOBX Metrics**: http://localhost:9000/metrics

## Detailed Setup

### Step 1: Configure Prometheus

The Prometheus configuration is in `monitoring/prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'lobx'
    static_configs:
      - targets: ['host.docker.internal:9000']
    scrape_interval: 5s
    metrics_path: '/metrics'
```

This configuration:
- Scrapes metrics from the LOBX application every 5 seconds
- Uses `host.docker.internal:9000` to connect from Docker to the host machine

### Step 2: Verify Prometheus is Scraping

1. Open http://localhost:9090/targets
2. Check that the `lobx` target shows as "UP"
3. If it shows "DOWN", verify that:
   - LOBX is running with `--features metrics-exporter`
   - The metrics endpoint is accessible at http://localhost:9000/metrics

### Step 3: Configure Grafana

The Grafana setup includes:
- **Datasource**: Automatically configured to use Prometheus
- **Dashboard**: Pre-configured with latency visualizations

#### Manual Datasource Setup (if needed)

1. Go to http://localhost:3000
2. Login with `admin`/`admin`
3. Go to ⚙️ Configuration → Data sources
4. Add new datasource → select Prometheus
5. Set URL to: `http://prometheus:9090`
6. Save & Test

### Step 4: View the Dashboard

The dashboard includes 5 panels:

1. **Order Submission Latency**: Overall submission latency (avg, p50, p90, p99)
2. **Limit Order Execution Latency**: Limit order processing latency
3. **Market Order Execution Latency**: Market order processing latency
4. **Order Matching Latency**: Core matching algorithm latency
5. **Order Submission Rate**: Orders per second

## PromQL Queries

Here are the key PromQL queries used in the dashboard:

### Average Latency
```promql
rate(lobx_submit_latency_ns_sum[5m]) / rate(lobx_submit_latency_ns_count[5m])
```

### Percentile Latencies
```promql
# p50
lobx_submit_latency_ns{quantile="0.5"}

# p90
lobx_submit_latency_ns{quantile="0.9"}

# p99
lobx_submit_latency_ns{quantile="0.99"}
```

### Order Rate
```promql
rate(lobx_submit_total[5m])
```

## Customization

### Adding New Metrics

1. Add metrics to your Rust code using the `metrics` crate
2. Restart the LOBX application
3. The new metrics will automatically appear in Prometheus
4. Add them to Grafana by creating new panels

### Modifying the Dashboard

1. Go to the dashboard in Grafana
2. Click "Settings" → "JSON Model"
3. Modify the JSON configuration
4. Save the changes

### Adjusting Scrape Intervals

Edit `monitoring/prometheus.yml`:
```yaml
scrape_configs:
  - job_name: 'lobx'
    scrape_interval: 1s  # More frequent scraping
```

Then reload Prometheus:
```bash
curl -X POST http://localhost:9090/-/reload
```

## Troubleshooting

### Prometheus Can't Connect to LOBX

**Problem**: Target shows as DOWN in http://localhost:9090/targets

**Solutions**:
1. Verify LOBX is running: `curl http://localhost:9000/metrics`
2. Check if metrics-exporter feature is enabled
3. For Docker on Mac/Windows, use `host.docker.internal:9000`
4. For Linux, use `172.17.0.1:9000` or your host IP

### No Data in Grafana

**Problem**: Dashboard shows "No data"

**Solutions**:
1. Check Prometheus has data: http://localhost:9090/graph
2. Verify time range in Grafana (top-right corner)
3. Check that LOBX is generating metrics (submit some orders)
4. Verify Prometheus datasource URL in Grafana

### High Memory Usage

**Problem**: Prometheus using too much memory

**Solutions**:
1. Increase scrape interval in `prometheus.yml`
2. Reduce retention time:
   ```yaml
   global:
     storage.tsdb.retention.time: 7d
   ```
3. Add resource limits to Docker Compose:
   ```yaml
   prometheus:
     deploy:
       resources:
         limits:
           memory: 1G
   ```

## Performance Considerations

### Metric Cardinality

- Keep metric labels minimal to avoid high cardinality
- Use histograms for latency measurements
- Avoid creating metrics per order ID

### Storage

- Prometheus data is stored in `/prometheus` volume
- Default retention is 15 days
- Monitor disk usage with `du -sh /var/lib/docker/volumes/`

### Network

- Scrape interval of 5s provides good balance of freshness vs. load
- Use `rate()` functions to smooth out counter metrics
- Consider using `increase()` for cumulative metrics

## Security

### Production Deployment

For production use:

1. **Change default passwords**:
   ```yaml
   environment:
     - GF_SECURITY_ADMIN_PASSWORD=your-secure-password
   ```

2. **Enable authentication**:
   ```yaml
   environment:
     - GF_AUTH_ANONYMOUS_ENABLED=false
   ```

3. **Use HTTPS**:
   ```yaml
   environment:
     - GF_SERVER_PROTOCOL=https
     - GF_SERVER_CERT_FILE=/etc/ssl/certs/grafana.crt
     - GF_SERVER_CERT_KEY=/etc/ssl/private/grafana.key
   ```

4. **Restrict network access**:
   ```yaml
   ports:
     - "127.0.0.1:3000:3000"  # Only localhost
   ```

## Monitoring Best Practices

1. **Set up alerts** for high latency percentiles
2. **Monitor error rates** (if you add error metrics)
3. **Track throughput** to understand system load
4. **Use consistent time windows** for rate calculations
5. **Regular backup** of Grafana dashboards and Prometheus rules

## Example Alerts

Add these to `monitoring/alerts.yml`:

```yaml
groups:
  - name: lobx
    rules:
      - alert: HighLatency
        expr: lobx_submit_latency_ns{quantile="0.99"} > 1000000  # 1ms
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High order submission latency"
          
      - alert: NoOrders
        expr: rate(lobx_submit_total[5m]) == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "No orders being processed"
```

## Support

For issues with the monitoring setup:

1. Check Docker logs: `docker-compose -f docker-compose.monitoring.yml logs`
2. Verify Prometheus targets: http://localhost:9090/targets
3. Test metrics endpoint: `curl http://localhost:9000/metrics`
4. Check Grafana logs in the container
