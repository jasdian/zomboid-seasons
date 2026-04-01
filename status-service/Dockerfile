FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY zomboid-status /app/zomboid-status
COPY status-service/static/ /app/static/
COPY status-service/migrations/ /app/migrations/
RUN mkdir -p /data/status-db && chmod +x /app/zomboid-status
EXPOSE 3001
ENTRYPOINT ["/app/zomboid-status", "/app/config.toml"]
