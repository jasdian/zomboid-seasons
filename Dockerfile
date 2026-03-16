FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl sudo systemd-sysv dbus \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY zomboid-seasons /app/zomboid-seasons
COPY static/ /app/static/
COPY migrations/ /app/migrations/
RUN mkdir -p /data/db /data/archive
RUN chmod +x /app/zomboid-seasons
EXPOSE 3000
ENTRYPOINT ["/app/zomboid-seasons", "/app/config.toml"]
