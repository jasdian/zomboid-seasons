-- ── Components ──
CREATE TABLE component_groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE components (
    id TEXT PRIMARY KEY,
    group_id TEXT REFERENCES component_groups(id),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL DEFAULT 0,
    status_override TEXT CHECK(status_override IN
        ('operational','degraded','partial_outage','major_outage','maintenance')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ── Health checks ──
CREATE TABLE probe_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    component_id TEXT NOT NULL REFERENCES components(id),
    success INTEGER NOT NULL,            -- 0 or 1
    response_ms INTEGER,
    error_message TEXT,
    checked_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_pr_component_time ON probe_results(component_id, checked_at);

-- ── Uptime ──
CREATE TABLE component_daily_uptime (
    component_id TEXT NOT NULL REFERENCES components(id),
    date TEXT NOT NULL,
    total_checks INTEGER NOT NULL DEFAULT 0,
    successful_checks INTEGER NOT NULL DEFAULT 0,
    uptime_seconds INTEGER NOT NULL DEFAULT 86400,
    worst_status TEXT NOT NULL DEFAULT 'operational'
        CHECK(worst_status IN ('operational','degraded','partial_outage','major_outage','maintenance')),
    incident_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (component_id, date)
);
CREATE INDEX idx_cdu_date ON component_daily_uptime(date);

-- ── Incidents ──
CREATE TABLE incidents (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('investigating','identified','monitoring','resolved')),
    severity TEXT NOT NULL CHECK(severity IN ('minor','major','critical')),
    auto_detected INTEGER NOT NULL DEFAULT 0,    -- was this created by a probe?
    postmortem_body TEXT,
    postmortem_published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT
);
CREATE INDEX idx_i_status ON incidents(status);
CREATE INDEX idx_i_created ON incidents(created_at);

CREATE TABLE incident_updates (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK(status IN ('investigating','identified','monitoring','update','resolved')),
    message TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_iu_incident ON incident_updates(incident_id, created_at);

CREATE TABLE incident_components (
    incident_id TEXT NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    component_id TEXT NOT NULL REFERENCES components(id),
    PRIMARY KEY (incident_id, component_id)
);

-- ── Maintenance ──
CREATE TABLE scheduled_maintenances (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK(status IN ('scheduled','in_progress','completed')),
    component_ids TEXT NOT NULL,          -- JSON array
    scheduled_start TEXT NOT NULL,
    scheduled_end TEXT NOT NULL,
    actual_start TEXT,
    actual_end TEXT,
    auto_suppress_incidents INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_sm_status ON scheduled_maintenances(status);

-- ── Subscribers ──
CREATE TABLE subscribers (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL CHECK(type IN ('webhook','discord')),
    endpoint TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
