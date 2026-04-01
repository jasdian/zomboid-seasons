# Project Zomboid Multiplayer Reward System — Full Plan

## Context

- Project Zomboid B42.15 unstable multiplayer (WIP, ≤20 players recommended)
- Server wipes every **90 real days** (one "season")
- Game-time ratio: **24h game = 1.5h real**
- Zombie kill tracking already exists (leaderboard)
- Rewards are claimed via an **external website**, not in-game UI
- Backend: Rust / Axum / SQLite (or Postgres)
- Game integration: RCON + server log parsing

---

## Three-Layer Model

### Character (ephemeral)

Lives and dies within a season. Identified by a **server-assigned ULID** (not by in-game name, which is non-unique). Tracks zombie kills and game-days survived. On death (or season end if still alive), becomes a frozen **snapshot**. A player may have many characters per season, including multiple with the same name.

### Season (competitive)

90 real days. Holds all character snapshots for a player plus their daily login counter. The **leaderboard** reads from this layer. Season score is computed from the player's best character plus their login consistency. Displayed on the website in real time.

### Account (persistent)

Permanent identity keyed by **Steam ID**. At season close, the season score is credited to a lifetime balance. Balance carries across seasons, is spendable on the website, never expires.

---

## Scoring Formulas

Three independent point sources per season, rewarding different behaviors.

### Online — loyalty (show up consistently)

```
P_online = 10 × days_logged
```

- Flat **10 points per day** where the player was online ≥1 hour
- Uncapped (max ~900 over a 90-day season)
- Rewards: consistent attendance, not AFK grinding

### Kills — engagement (fight zombies)

```
P_kills = 500 × (1 − e^(−kills / 200))
```

- Non-linear, front-loaded: first kills are worth more
- Hard input cap at **1000 kills** (kills beyond 1000 are ignored)
- Max theoretical output: **~497 points**
- Prevents kill-farming from dominating the leaderboard

| Kills | Points |
|------:|-------:|
| 50    | 110    |
| 100   | 197    |
| 200   | 316    |
| 500   | 459    |
| 1000  | 497    |

### Survival — skill (stay alive)

```
P_survival = 700 × (days / 30)²
```

- Non-linear, back-loaded: later days are worth exponentially more
- Hard input cap at **30 game-days**
- Max theoretical output: **700 points**
- Dying on day 29 is devastating — this is intentional

| Game-Days | Points |
|----------:|-------:|
| 5         | 19     |
| 10        | 78     |
| 15        | 175    |
| 20        | 311    |
| 25        | 486    |
| 30        | 700    |

### Season Total

```
Season Score = P_online + P_kills(best_char) + P_survival(best_char)
```

Kills and survival come from the **single best character** in that season, selected by highest combined `P_kills + P_survival`. Online points accumulate independently across the whole season regardless of character deaths.

---

## Best Character Selection

```
best_character = argmax over all characters in season (P_kills(c) + P_survival(c))
```

Examples:

- A 30-day survivor with 50 kills: 110 + 700 = **810** (selected)
- A character with 1000 kills who died on day 2: 497 + 3 = **500** (not selected)

Survival dominance is intentional. This is Project Zomboid.

### Design Decision: Combined vs Separate Domains

**Considered alternatives:**

- **Separate domains** — best killer character provides kill score, best survivor character provides survival score independently. Rewards peak performance in each area, but removes tension: players can suicide-rush a kill run, die, then play safe for survival. The two goals stop competing, which removes a meaningful in-game decision.
- **Same character can count for both** — a middle ground, but still eliminates the tradeoff between aggressive and cautious play.

**Chosen: Combined (single best character provides both scores).**

This forces a real tradeoff. A 25-day survivor with zero kills scores 0 + 486 = 486. A day-5 character with 500 kills scores 459 + 19 = 478. Past ~22 game-days, survival overtakes even heavy killers — this means kills are most impactful for players who die before the survival curve kicks in, creating a natural consolation prize for aggressive play. The core tension — "do I risk my high-value survivor on that dangerous loot run?" — is preserved. That tension *is* Project Zomboid.

---

## Player Profiles Across a Season

| | Casual | Regular | Hardcore |
|---|-------:|--------:|---------:|
| Days logged | 30 | 60 | 80 |
| Kills (best char) | 200 | 500 | 1000 |
| Best survival | 10 days | 20 days | 30 days |
| **Online pts** | 300 | 600 | 800 |
| **Kill pts** | 316 | 459 | 497 |
| **Survival pts** | 78 | 311 | 700 |
| **Season Total** | **694** | **1370** | **1997** |

Hardcore-to-casual ratio: ~3:1. A casual player who keeps one character alive for 30 days scores 1316, nearly matching a regular player. Survival is the great equalizer.

---

## Account Balance — Cross-Season Accumulation

At season close, the season score is credited to the player's account. Balance persists forever, is spendable on the website.

**Ledger pattern, not a mutable balance column.** Every credit (season settlement) and debit (redemption) is an append-only row. Balance = sum of all entries for that Steam ID. This provides a full audit trail, prevents drift bugs, and makes disputes trivial to investigate.

```
Balance = SUM(amount) WHERE steam_id = X
```

Settlement is **season-end only** (v1). No mid-season spending. This is simpler, prevents edge cases where someone spends points and then the best-character snapshot changes.

---

## Architecture

```
┌──────────────┐    RCON / log parse     ┌───────────────┐
│  PZ Server   │ ──────────────────────► │  RCON Listener │
│  (B42.15)    │ ◄────────────────────── │  (Rust agent)  │
└──────────────┘    RCON commands        └───────┬───────┘
                                                 │ HTTP
                                                 ▼
                                         ┌───────────────┐
                                         │  Axum API      │
                                         │  (scoring,     │
                                         │   leaderboard, │
                                         │   redemption)  │
                                         └───────┬───────┘
                                                 │
                                         ┌───────▼───────┐
                                         │  Website       │
                                         │  (Steam auth,  │
                                         │   leaderboard, │
                                         │   reward shop) │
                                         └───────────────┘
```

The RCON listener captures game events (kills, logins, logouts, deaths, character creation) and posts them to the Axum API. The API writes to the database and serves the website. Steam OpenID authenticates players on the website, tying their web session to their Steam ID.

Redemptions queue an RCON command for the next time the player is online in-game (e.g., item spawn via `additem`).

---

## Storage Design

### Principle: append-only events as source of truth

Don't write scores directly. Write raw game events into an append-only log. Scores, leaderboards, and balances are all **derived** from these events. If you tweak scoring curves between seasons, you can re-derive without data loss.

### Five Storage Concepts

**1. Accounts** — one row per Steam ID. Display name, created_at. No balance column (balance is derived from the ledger).

**2. Seasons** — one row per season. Season number, start date, end date, finalized flag.

**3. Events** — append-only, single source of truth. Each row:

- Event ID (ULID)
- Steam ID
- Season ID
- **Character ID (ULID)** — synthetic, assigned by the listener (NULL for LOGIN/LOGOUT if no active character)
- Event type: `KILL`, `LOGIN`, `LOGOUT`, `DEATH`, `CHARACTER_CREATE`
- Payload (JSON): character name (display only), kill count delta, game-days survived, cause of death, etc.
- Source idempotency key (`UNIQUE` — `ON CONFLICT DO NOTHING`)
- Ingested-at timestamp

**4. Character Snapshots** — materialized from events on character death or season close. Used for leaderboard reads and best-character selection.

- **Character ID (ULID)** — synthetic, assigned server-side by the RCON listener (see below)
- Steam ID, season ID
- Character name (display only — **not unique**, same player may reuse names across characters)
- Total kills, survived game-days
- Died-at (NULL if still alive at season close)

These are derived checkpoints, not the source of truth. Can be rebuilt from the event log.

**Character identity is synthetic, not name-based.** In-game character names are not unique — the same Steam account can create multiple characters with the same name during a season. The RCON listener assigns a new ULID when it detects a new character (CHARACTER_CREATE event or first login after a DEATH event for that Steam ID). All subsequent events are tagged with this ULID until the next death triggers a new one. Detection heuristic: if a player connects and their most recent character has a DEATH event, mint a new ID; if the last character has no death, reuse the existing ID (reconnect scenario).

**5. Balance Entries** — append-only ledger.

- Entry ID
- Steam ID
- Amount (positive = credit, negative = debit)
- Entry type: `SEASON_CREDIT` or `REDEMPTION`
- Reference ID (season ID or redemption ID)
- Created-at timestamp

---

## Key Queries (Conceptual)

**Leaderboard:** Join character snapshots with login-day counts (derived from LOGIN/LOGOUT events). Apply scoring formulas in the query. Select each player's best character by `argmax(P_kills + P_survival)`. Order by total season score descending.

**Account balance:** `SUM(amount) FROM balance_entries WHERE steam_id = ?`

**Season settlement:** In one transaction: compute final scores for all players, INSERT into balance_entries as SEASON_CREDIT rows, SET season.finalized = true. Idempotent — the finalized flag prevents double-credit on re-run.

---

## Anti-Exploit Considerations

**Duplicate events.** The idempotency key (`UNIQUE` constraint with `ON CONFLICT DO NOTHING`) on the events table handles duplicate RCON messages from reconnects, retries, or log replays. No application-level deduplication needed.

**Double settlement.** The finalized flag on the season table, checked within the settlement transaction, prevents crediting a season twice. The settlement job is idempotent and re-runnable.

**Balance race conditions.** Redemptions compute balance via `SUM(amount)` at debit time, not from a cached column. At this player count, row-level locking during the redemption transaction is sufficient — no optimistic locking complexity needed.

**AFK farming (online points).** The 1-hour minimum per day prevents idle connections from farming. Can tighten later by requiring movement or interaction events, but start simple.

**Kill botting.** The exponential saturation curve means diminishing returns past ~200 kills. Even at the hard cap of 1000, kills contribute only 497 points — less than a 27-day survivor gets from survival alone. Kill farming is self-limiting by design.

---

## Reward Catalog (v1 Proposal)

Spendable on the website, delivered via RCON next time the player is online.

| Tier | Cost | Contents | Notes |
|------|-----:|----------|-------|
| Care Package | ~200 pts | Backpack, can opener, canned food, flashlight, batteries | Saves early-game tedium |
| Utility Bundle | ~600 pts | Watch, good axe, cooking pot, first aid kit, thread + needle | Mid-tier utility, no weapons |
| Cosmetic | ~1000 pts | Exclusive clothing item (SWAT uniform, football outfit, etc.) | B42 added many new clothing items |
| Prestige | ~3000 pts | Unique cosmetic + Discord role + Hall of Fame entry | Takes 2+ seasons to afford |

**No cars. No guns.** These break the survival loop, especially early-wipe. Rewards should remove tedium, not trivialize progression.

Limit: **one redemption per category per character.** Die and you can redeem again on your next character.

---

## Season Lifecycle

1. **Season starts.** Server wipe, fresh map. Season record created with start date.
2. **During season.** RCON listener captures events. Website shows live leaderboard (projected scores using currently-best character). Players cannot spend points (v1).
3. **Season ends (day 90).** Settlement job runs: freeze all alive characters as snapshots, compute final scores, credit accounts, mark season finalized.
4. **Between seasons.** Players can browse the reward shop, spend accumulated balance, redeem rewards. Rewards queue for delivery when the new season starts and they connect.
5. **New season starts.** Cycle repeats. Account balances carry forward.

---

## Open Decisions for Later

- **Mid-season spending.** Could allow online points to be spendable during the season. Adds complexity — best character might change after points are spent.
- **Decay / expiration.** Currently points never expire. Could add seasonal decay (e.g., lose 10% of unspent balance per season) to encourage spending. Not recommended for v1.
- **Team/faction rewards.** Track group accomplishments (e.g., base building, group survival). Requires defining faction mechanics first.
- **Voting rewards.** Like the PZ Pet Discord bot — reward players for voting on server listing sites. Easy bolt-on, uses the same balance ledger.
- **Formula tuning.** The curves (especially the 200-kill inflection and 30-day survival cap) will need adjustment based on actual play data from the first season. The event-sourced design makes re-derivation trivial.
