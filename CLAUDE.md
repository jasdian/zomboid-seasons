# CLAUDE.md

## Project Overview

Rust backend for a seasonal Project Zomboid server with Ethereum payment integration. Manages player registrations, promo codes, automated 90-day season rotation, and ETH payment verification. Single access tier: pay = whitelisted immediately.

## Tech Stack

- **Language:** Rust (latest stable)
- **Web framework:** Axum (async HTTP on Tokio)
- **Database:** SQLite via sqlx (async, compile-time checked queries)
- **Blockchain:** Alloy (Ethereum RPC client)
- **Server control:** RCON protocol to PZ dedicated server
- **Logging:** `tracing` + `tracing-subscriber` with JSON output
- **Dev environment:** Nix (`nix-shell` to enter)

## Build Commands

```bash
nix-shell
cargo build
cargo run
cargo build --release --no-default-features --target x86_64-unknown-linux-musl
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

## Architecture

### Key Domain Concepts

- **Single access tier:** Pay -> whitelisted immediately. No spectator system.
- **Carry-forward:** Top zombie killers earn free registration in the next season (configurable count). Everyone else must re-register and pay.
- **Payment matching:** Single deposit address + unique wei offset per registration.
- **Password:** PZ whitelist requires username+password. Stored AES-256-GCM encrypted at rest in DB; decrypted only when needed for RCON commands. Key configured via `database.encryption_key` (base64 of 32 bytes).
- **Season rotation:** Every 90 days. Stop server -> archive save dir as .tar.gz -> create new season -> carry forward top killers -> delete save dir (PZ auto-generates) -> update .ini -> restart.

### Background Services

1. **ETH payment poller** (~15s): scans new blocks for matching transfers, whitelists on confirm
2. **Season rotation scheduler** (check every 60s): 90-day trigger
3. **Registration expiry cleanup** (10min): marks stale AwaitingPayment as Expired

### API Structure

- **Public:** `POST /api/register`, `GET /api/season`, `GET /api/seasons`, `GET /api/register/{id}`, `GET /api/saves/{season_id}`
- **Admin** (bearer token): `POST/GET/DELETE /api/admin/promo`, `POST /api/admin/rotate`, `GET /api/admin/registrations`, `DELETE /api/admin/registrations/{id}`

### Database Tables

Three tables: `seasons` (id, status, dates, save_path), `registrations` (UUID id, season_id, zomboid_username, zomboid_password, eth_address, tx_hash, promo_code, status, amount_wei, timestamps), `promo_codes` (code, discount_percent, max_uses, times_used, active, timestamps).

### PZ Server Management

- RCON commands: `adduser "user" "pass"`, `removeuserfromwhitelist "user"`, `save`, `quit`, `help`
- Config: `~/Zomboid/Server/<server_name>.ini` (update `PublicName=`)
- Saves: `~/Zomboid/Saves/Multiplayer/<server_name>/` (directory, not single file)
- Wipe: delete save directory, PZ generates new world on next start
- **Server-side Lua:** Deployed directly into PZ's `~/pzserver/media/lua/server/ZomboidSeasons/` (not via `Mods=`, which forces clients to install). Rotation code strips `ZomboidSeasons` from `Mods=` if present.
- **PZ Lua file sandbox:** `getFileWriter`/`getFileReader` resolve paths relative to `~/Zomboid/Lua/`. Mod data files (kills.json, drops_*.json, config.json) live under `~/Zomboid/Lua/ZomboidSeasons/`. Never use absolute paths in Lua mod file I/O.

## Deployment

- Backend runs as dedicated user with sudoers permission for `zomboid.service`
- Config in TOML format
- Logs via `journalctl -u zomboid-seasons -o json | jq`
