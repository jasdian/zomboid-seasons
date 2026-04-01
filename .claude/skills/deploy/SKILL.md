---
name: deploy
description: "Build release binary, deploy to server, restart service."
---

# Deploy — Build, Ship, Restart

Deploy the zomboid-seasons backend to production using `scripts/deploy.sh`.

## Steps

### 1. Pre-flight Check
Run `/test` to verify everything passes. Do NOT proceed if tests fail.

### 2. Deploy
Run the deploy script which builds, copies files, and restarts the container:
```bash
nix-shell --run ./scripts/deploy.sh
```

Options:
- `--mod-only` — hot-reload Lua files only (no binary rebuild)
- `--reset-db` — backup + flatten + recreate DB

### 3. Verify
The script checks container health automatically. Confirm the output shows:
- "Backend is healthy"
- "Deploy complete"

### 4. Report
- Service status (healthy/failed)
- Any warnings from startup logs
