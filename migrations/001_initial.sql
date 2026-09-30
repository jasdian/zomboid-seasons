-- Zomboid Seasons — unified schema
-- Includes: core tables, status page, reward system, shop (deferred)

-- ── Core ──

CREATE TABLE seasons (
    id INTEGER PRIMARY KEY,
    status TEXT NOT NULL CHECK(status IN ('pending','active','archived')),
    started_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    save_path TEXT,
    finalized INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE registrations (
    id TEXT PRIMARY KEY,
    season_id INTEGER NOT NULL REFERENCES seasons(id),
    zomboid_username TEXT NOT NULL,
    zomboid_password TEXT NOT NULL,
    eth_address TEXT NOT NULL,
    steam_id TEXT NOT NULL DEFAULT '',
    promo_code TEXT,
    tx_hash TEXT,
    status TEXT NOT NULL CHECK(status IN ('awaiting_payment','confirmed','expired')),
    amount_wei TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    confirmed_at TEXT
);

CREATE TABLE promo_codes (
    code TEXT PRIMARY KEY,
    discount_percent INTEGER NOT NULL CHECK(discount_percent BETWEEN 0 AND 100),
    max_uses INTEGER,
    times_used INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT
);

CREATE TABLE player_stats (
    season_id        INTEGER NOT NULL,
    steam_id         TEXT    NOT NULL,
    zomboid_username TEXT    NOT NULL,
    zombie_kills     INTEGER NOT NULL DEFAULT 0,
    last_synced_at   TEXT    NOT NULL,
    PRIMARY KEY (season_id, zomboid_username),
    FOREIGN KEY (season_id) REFERENCES seasons(id)
);

CREATE TABLE supply_drops (
    id              TEXT PRIMARY KEY,
    season_id       INTEGER NOT NULL,
    poi_name        TEXT NOT NULL,
    location_x      INTEGER NOT NULL,
    location_y      INTEGER NOT NULL,
    location_z      INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL CHECK(status IN ('scheduled','announced','active','claimed','expired')),
    loot_tier       TEXT NOT NULL DEFAULT 'standard',
    scheduled_at    TEXT NOT NULL,
    announced_at    TEXT,
    activated_at    TEXT,
    claimed_by_steam_id TEXT,
    claimed_by_username TEXT,
    claimed_at      TEXT,
    expires_at      TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (season_id) REFERENCES seasons(id)
);

-- ── Status page ──

CREATE TABLE status_components (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL DEFAULT 0,
    status_override TEXT CHECK(status_override IN ('operational','degraded','major_outage')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE status_incidents (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('investigating','identified','monitoring','resolved')),
    severity TEXT NOT NULL CHECK(severity IN ('minor','major','critical')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT
);

CREATE TABLE status_incident_updates (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES status_incidents(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK(status IN ('investigating','identified','monitoring','update','resolved')),
    message TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE status_incident_components (
    incident_id TEXT NOT NULL REFERENCES status_incidents(id) ON DELETE CASCADE,
    component_id TEXT NOT NULL REFERENCES status_components(id),
    PRIMARY KEY (incident_id, component_id)
);

-- ── Reward system ──

CREATE TABLE accounts (
    steam_id     TEXT PRIMARY KEY,
    display_name TEXT NOT NULL DEFAULT '',
    balance      INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE game_events (
    id              TEXT PRIMARY KEY,
    steam_id        TEXT NOT NULL,
    season_id       INTEGER NOT NULL REFERENCES seasons(id),
    character_id    TEXT,
    event_type      TEXT NOT NULL CHECK(event_type IN (
        'character_created', 'zombie_kill', 'player_death',
        'login', 'logout', 'season_close'
    )),
    payload         TEXT NOT NULL DEFAULT '{}',
    idempotency_key TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(idempotency_key)
);

CREATE TABLE character_snapshots (
    id                   TEXT PRIMARY KEY,
    steam_id             TEXT NOT NULL,
    season_id            INTEGER NOT NULL REFERENCES seasons(id),
    zomboid_username     TEXT NOT NULL,
    zombie_kills         INTEGER NOT NULL DEFAULT 0,
    game_days_survived   REAL NOT NULL DEFAULT 0.0,
    created_at_world_age REAL NOT NULL DEFAULT 0.0,
    died_at              TEXT,
    p_kills              REAL NOT NULL DEFAULT 0.0,
    p_survival           REAL NOT NULL DEFAULT 0.0,
    created_at           TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at           TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE balance_entries (
    id           TEXT PRIMARY KEY,
    steam_id     TEXT NOT NULL,
    amount       INTEGER NOT NULL,
    entry_type   TEXT NOT NULL CHECK(entry_type IN (
        'season_score', 'shop_purchase', 'admin_grant', 'admin_deduct'
    )),
    reference_id TEXT,
    description  TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE online_days (
    steam_id       TEXT NOT NULL,
    season_id      INTEGER NOT NULL,
    date           TEXT NOT NULL,
    minutes_online INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (steam_id, season_id, date)
);

-- ── Shop (deferred, schema ready) ──

CREATE TABLE shop_items (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    description       TEXT NOT NULL DEFAULT '',
    cost              INTEGER NOT NULL,
    category          TEXT NOT NULL,
    rcon_commands     TEXT NOT NULL DEFAULT '[]',
    max_per_character INTEGER DEFAULT 1,
    active            INTEGER NOT NULL DEFAULT 1,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE shop_redemptions (
    id           TEXT PRIMARY KEY,
    steam_id     TEXT NOT NULL,
    item_id      TEXT NOT NULL REFERENCES shop_items(id),
    character_id TEXT,
    season_id    INTEGER NOT NULL,
    delivered    INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ── Indexes ──

CREATE INDEX idx_reg_season     ON registrations(season_id);
CREATE INDEX idx_reg_status     ON registrations(status);
CREATE INDEX idx_reg_eth        ON registrations(eth_address);
CREATE INDEX idx_reg_amount     ON registrations(amount_wei);
CREATE INDEX idx_reg_name       ON registrations(zomboid_username);
CREATE INDEX idx_reg_steam_id   ON registrations(steam_id);
CREATE INDEX idx_promo_active   ON promo_codes(active);
CREATE INDEX idx_ps_kills       ON player_stats(season_id, zombie_kills DESC);
CREATE INDEX idx_sd_season_status ON supply_drops(season_id, status);
CREATE INDEX idx_sd_status      ON supply_drops(status);
CREATE INDEX idx_si_status      ON status_incidents(status);
CREATE INDEX idx_si_created     ON status_incidents(created_at);
CREATE INDEX idx_siu_incident   ON status_incident_updates(incident_id, created_at);
CREATE INDEX idx_ge_steam_season ON game_events(steam_id, season_id);
CREATE INDEX idx_ge_character   ON game_events(character_id);
CREATE INDEX idx_ge_type_season ON game_events(event_type, season_id);
CREATE INDEX idx_cs_steam_season ON character_snapshots(steam_id, season_id);
CREATE INDEX idx_cs_season_score ON character_snapshots(season_id, p_kills, p_survival);
CREATE INDEX idx_be_steam       ON balance_entries(steam_id);
CREATE INDEX idx_sr_steam       ON shop_redemptions(steam_id, item_id);
CREATE INDEX idx_sr_pending     ON shop_redemptions(delivered) WHERE delivered = 0;

-- ── Seed data ──

INSERT INTO seasons (id, status, started_at, ends_at)
VALUES (1, 'active', datetime('now'), datetime('now', '+90 days'));

INSERT INTO status_components (id, name, description, position) VALUES
    ('game_server', 'Game Server', 'Project Zomboid dedicated server', 1),
    ('backend_api', 'Backend API', 'Registration and payment services', 2),
    ('payments', 'Payment Processing', 'Ethereum payment verification', 3);
