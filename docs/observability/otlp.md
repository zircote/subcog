# OTLP Exporter Settings

## Endpoints

- `SUBCOG_OTLP_ENDPOINT` (preferred)
- `OTEL_EXPORTER_OTLP_ENDPOINT` (fallback)

## Protocol

- `SUBCOG_OTLP_PROTOCOL` (preferred)
- `OTEL_EXPORTER_OTLP_PROTOCOL` (fallback)

Supported values:

- `grpc`
- `http` / `http/protobuf`

## Defaults

- If the endpoint contains `:4317`, gRPC is selected.
- Otherwise HTTP/protobuf is used.
