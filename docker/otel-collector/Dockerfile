FROM otel/opentelemetry-collector-contrib:0.104.0

COPY docker/otel-collector/otelcol-config.yaml /etc/otelcol-contrib/config.yaml

# Run as non-root user (uid 10001 from base image)
USER 10001
