# Status System — Implementation Status & Roadmap

What we've built, what's live, and what's next.

---

## Implemented (Live in Production)

### Standalone Service Architecture
- Independent `zomboid-status` binary in Cargo workspace (`status-service/`)
- Own SQLite database, TOML config, Docker container on `status.princeofcrypto.com`
- Nginx reverse proxy with auto-SSL via `VIRTUAL_HOST` + letsencrypt companion
- Separate deploy script (`scripts/deploy-status.sh`)
- Rate limiting with `X-Real-IP` extraction behind nginx

### Health Probes (4 types)
| Type | Implementation |
|---|---|
| `http` | GET/HEAD with expected status, timeout, reqwest client |
| `tcp` | `TcpStream::connect` with `tokio::time::timeout` |
| `rcon` | `spawn_blocking` + `rcon_client`, wrapped in outer timeout |
| `json_rpc` | POST JSON-RPC 2.0, check for `result` field |

Config-driven per component via `[components.probe]` in TOML. Supports `password_file` for RCON secrets. Environment variable expansion (`${VAR}`) in config values.

### Auto-Incident Lifecycle
- Consecutive failure threshold → auto-create incident (`auto_detected=true`)
- Severity escalation: `failure_threshold` → minor, `2x` → major, `4x` → critical
- Recovery threshold → auto-resolve with update message
- Manual incidents take priority (suppress auto-resolution)
- Crash recovery: probe runner checks DB for existing auto-incidents on startup
- Maintenance suppression: skip auto-creation during active maintenance windows

### Component Statuses (5 levels)
`operational` → `degraded` → `partial_outage` → `major_outage` → `maintenance`

Derived from: manual override > active incident severity > operational default.

### Uptime Tracking
- Probe results recorded in `probe_results` table
- 5-minute aggregator rolls into `component_daily_uptime`
- Hourly cleanup prunes raw results older than 7 days
- **API**: `GET /api/status/uptime` (30-day per-component), `GET /api/status/uptime/calendar` (monthly heatmap)

### Uptime Visualization (Frontend)
- **30-day bar chart** on main status page — per-component, date-aligned, color-coded, hover tooltips
- **Calendar heatmap** (`/uptime.html`) — component selector, 3-month view, month navigation, day-level grid
- Color thresholds: 100% green, 99%+ yellow-green, 95%+ orange, <95% red, no data gray

### Incident History
- **API**: `GET /api/status/history` with `from`, `to`, `components` filter, `expand_month`, `preview_count`
- **Frontend** (`/history.html`): monthly grouping, component filter checkboxes, expand/collapse, duration display

### Scheduled Maintenance
- Admin create/patch via API, internal endpoint for main backend (`POST /internal/maintenance`)
- Maintenance watcher (60s loop): auto-transitions `scheduled` → `in_progress` → `completed`
- Sets component `status_override` to `maintenance` during active window, clears on completion
- "Start Now" adjusts `scheduled_end` forward if already past (prevents immediate auto-complete)
- Public maintenance banner on status page during active/scheduled windows
- Admin UI shows all maintenances including completed (dimmed)

### Admin UI (`/admin.html`)
- Bearer token auth (shared with main backend)
- Incident CRUD: create with severity + components, post updates, resolve, delete
- Component status overrides (manual override dropdown)
- Maintenance scheduling: title, description, components, start/end times, start now, complete
- Postmortem publishing on resolved incidents

### Postmortem Support
- Admin endpoint `POST /api/admin/incidents/{id}/postmortem` (body: `{ body: "..." }`)
- `IncidentResponse` includes optional `postmortem_body` and `postmortem_published_at` fields
- Displayed on status page, history page, incident detail page, and admin dashboard
- Admin UI shows resolved incidents with textarea to write and publish postmortems

### Component Grouping
- Config-driven via `[[component_groups]]` in TOML with `group_id` on components
- Groups upserted on startup (before components, for FK integrity)
- `GET /api/status` includes `component_groups` array; components carry `group_id`
- Frontend renders components grouped under headers on status page

### RSS/Atom Feed
- `GET /api/status/feed.atom` — Atom XML with recent 90-day incidents + active/upcoming maintenance
- No external dependencies — built via `format!()` string templating
- Content-Type: `application/atom+xml; charset=utf-8`

### Single Incident Detail
- `GET /api/status/incidents/{id}` — public endpoint with full incident, updates, components, postmortem
- Uses `tokio::join!` for parallel data fetching (updates + component IDs)
- Standalone `incident.html` detail page linked from history and status pages

### API Surface

**Public:**
| Endpoint | Status |
|---|---|
| `GET /api/status` | Live |
| `GET /api/status/uptime` | Live |
| `GET /api/status/uptime/calendar` | Live |
| `GET /api/status/history` | Live |
| `GET /api/status/maintenance` | Live |
| `GET /api/status/feed.atom` | Live |
| `GET /api/status/incidents/{id}` | Live |

**Admin (bearer token):**
| Endpoint | Status |
|---|---|
| `POST /api/admin/incidents` | Live |
| `GET /api/admin/incidents` | Live |
| `PATCH /api/admin/incidents/{id}` | Live |
| `DELETE /api/admin/incidents/{id}` | Live |
| `POST /api/admin/incidents/{id}/updates` | Live |
| `POST /api/admin/incidents/{id}/postmortem` | Live |
| `POST /api/admin/components/{id}/override` | Live |
| `POST /api/admin/maintenance` | Live |
| `GET /api/admin/maintenance` | Live |
| `PATCH /api/admin/maintenance/{id}` | Live |

**Internal (no auth, Docker-only):**
| Endpoint | Status |
|---|---|
| `POST /internal/maintenance` | Live |

### Background Tasks
| Task | Interval | Status |
|---|---|---|
| Component probes | 15-60s per config | Live |
| Auto-incident manager | On probe result | Live |
| Uptime aggregator | 5 min | Live |
| Maintenance watcher | 60s | Live |
| Probe result cleanup | 1 hour | Live |

---

## Not Yet Implemented

### P2: Discord/Webhook Subscriber Notifications
DB table `subscribers` exists in schema. When auto-incidents fire, POST to Discord webhooks so the community sees outages in real-time.

**What's needed:**
- `src/notifications.rs` — dispatch function called from probe runner on incident create/update/resolve
- Discord webhook: POST embed with incident title, severity, status, components
- Generic webhook: POST JSON payload with event type + incident data
- Admin endpoints: `POST /api/admin/subscribers`, `DELETE /api/admin/subscribers/{id}`
- Admin UI section for managing subscribers

**Effort:** Medium. **Value:** High — auto-incidents become truly useful with notifications.

### Future: `command` Probe Type
Execute arbitrary shell commands as health checks (exit code 0 = success).

**What's needed:**
- `CommandProbe` in `src/probes/command.rs`
- Config: `type = "command"`, `command = "..."`, `timeout_ms`
- Security consideration: only allow in config file, not via API

**Effort:** Low. **Value:** Low — niche use case.

---

## Quick Wins (next session)

1. **Discord webhook** — most impactful remaining feature, community gets real-time outage alerts
