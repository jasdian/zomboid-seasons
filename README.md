# Zomboid Seasons

Seasonal Project Zomboid server with Ethereum payment integration. Pay ETH to register, get whitelisted immediately. 90-day season rotation — top zombie killers earn free entry to the next season.

Requires **Build 42+** (currently running unstable until Build 42 reaches stable).

## Quick Start

```bash
nix-shell
cargo run -- test-config.toml
```

## Architecture

```
Internet -> Caddy/Nginx (TLS) -> zomboid-seasons :3000 -> RCON -> PZ Server
```

- **Backend** (`zomboid-seasons` container): Rust/Axum on port 3000
- **PZ Server** (`zomboid.service`): Project Zomboid Build 42+ dedicated server via SteamCMD
- **Database**: SQLite with WAL mode

## Build

```bash
# Development
cargo build && cargo run

# Production (static MUSL binary)
cargo build --release --no-default-features --target x86_64-unknown-linux-musl
```

## Configuration

Copy `config.prod.toml` and fill in:
- ETH RPC URL and deposit address
- RCON password file path
- Admin bearer token
- PZ server name and paths

## Deployment

All scripts run locally and execute commands on the remote server via SSH.

### First-time setup (in order)

```bash
# 1. Create pzuser, directories, firewall rules
./scripts/setup-server.sh

# 2. Create RCON password on server
ssh -p 55555 root@xxx.xxx.xxx.xxx 'openssl rand -hex 16 > /root/zomboid-seasons/rcon.pw'

# 3. Install PZ dedicated server via SteamCMD (unstable branch by default, requires Steam login)
./scripts/sync-zomboid.sh your_steam_username

# 4. Create systemd service with RCON
./scripts/setup-zomboid-service.sh

# 5. Deploy the Rust backend (Docker)
./scripts/deploy.sh
```

### Updating

```bash
# Update PZ server (stops service, updates via SteamCMD, restarts)
./scripts/sync-zomboid.sh your_steam_username

# Redeploy backend (builds binary, copies to server, restarts container)
./scripts/deploy.sh
```

### Scripts

| Script | Purpose | When to run |
|--------|---------|-------------|
| `setup-server.sh` | Create pzuser, directories, UFW rules | Once, on fresh server |
| `sync-zomboid.sh` | Install/update PZ server via SteamCMD (unstable branch by default) | Initial setup + game updates |
| `setup-zomboid-service.sh` | Create systemd unit, configure RCON | Once, after first sync |
| `deploy.sh` | Build and deploy Rust backend (Docker) | Every backend release |
| `deploy.sh --mod-only` | Rsync Lua mod + hot-reload via RCON (no restart) | Lua-only changes |
| `restart-pz.sh` | Restart PZ game server via systemctl | After .ini/sandbox changes |
| `switch-branch.sh` | Switch PZ branch (wipes saves, reinstalls, redeploys) | When Build 42 hits stable (switch from unstable to public) |
| `backup.sh` | Backup SQLite DB and config | Runs daily via cron (set up by deploy.sh) |

## PZ Server Configuration

### Directory structure

```
/home/pzuser/
  pzserver/                                PZ dedicated server (installed via apt steamcmd)
  Zomboid/
    mods/
      ZomboidSeasons/                      Server-side only Lua mod (kill tracking, events)
        42/                                Build 42 copy (required by B42 mod loader)
    Server/
      zomboid-seasons.ini                  Main server config
      zomboid-seasons_SandboxVars.lua      Gameplay tuning
    Saves/
      Multiplayer/
        zomboid-seasons/                   World save data (wiped on rotation)
    ZomboidSeasons/                        Mod runtime data (kills.json, drops, config)
```

### Key server.ini settings

| Setting | Description |
|---------|-------------|
| `PublicName=` | Server name in browser (managed by season rotation) |
| `PublicDescription=` | Server description |
| `MaxPlayers=` | Player cap |
| `PVP=true/false` | PvP toggle |
| `Open=false` | Whitelist mode: only `adduser` accounts can join (managed by backend) |
| `AutoCreateUserInWhiteList=false` | Prevent players from self-registering accounts (managed by backend) |
| `Password=` | Join password (empty = no password) |
| `PauseEmpty=true` | Pause when no players connected |
| `Map=Muldraugh, KY` | Map name |
| `DefaultPort=16261` | Game port (UDP) |
| `RCONPort=27016` | RCON port (set by setup-zomboid-service.sh) |
| `RCONPassword=` | RCON password (set by setup-zomboid-service.sh) |

### Sandbox settings

File: `/home/pzuser/Zomboid/Server/zomboid-seasons_SandboxVars.lua`

Controls gameplay mechanics: zombie population, loot respawn, day length, erosion speed, infection settings, etc. Generated on first server start with defaults. Edit while server is stopped. Delete the file to reset to defaults.

### DNS

Two types of DNS records are needed:

**A record** — points the domain to the server IP, used by the web backend (reverse proxy with TLS):

```
zomboid.example.com.  A  xxx.xxx.xxx.xxx
```

**SRV record** — allows game clients to discover the server by domain name and port:

```
_zomboid._udp.zomboid.example.com.  SRV  0 0 16261 zomboid.example.com.
```

Players connect using the domain name; the SRV record resolves the correct port.

### Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 16261 | UDP | Game traffic |
| 16262 | UDP+TCP | Steam query |
| 27016 | TCP | RCON (Docker-only, not exposed externally) |
| 3000 | TCP | Backend API (behind reverse proxy) |

### Firewall (UFW)

Configured by `setup-server.sh`:

```
55555/tcp    SSH
80/tcp       HTTP (reverse proxy)
443/tcp      HTTPS (reverse proxy)
16261/udp    PZ game port
16262         PZ Steam query port (UDP+TCP)
27016/tcp    RCON (restricted to Docker subnet 172.16.0.0/12)
```
