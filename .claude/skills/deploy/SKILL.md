---
name: deploy
description: "Build release binary, deploy to server, restart service."
disable-model-invocation: true
---

# Deploy — Build, Ship, Restart

Deploy the zomboid-seasons backend to production.

## Steps

### 1. Pre-flight Check
Run `/test` to verify everything passes. Do NOT proceed if tests fail.

### 2. Build Release Binary
```bash
cargo build --release --no-default-features --target x86_64-unknown-linux-musl
```

### 3. Confirm with User
Before deploying, show:
- Binary size and path
- Target server (from config or ask user)
- Current service status on remote

**Wait for explicit user confirmation before proceeding.**

### 4. Deploy via SSH
Based on the project's deployment setup:
```bash
# Copy binary
scp target/x86_64-unknown-linux-musl/release/zomboid-seasons user@server:/path/

# Copy static files
scp -r static/* user@server:/path/static/

# Copy mod files if changed
scp -r mod/* user@server:/path/mod/
```

Adjust paths based on user input or existing deployment config.

### 5. Restart Service
```bash
ssh user@server "sudo systemctl restart zomboid-seasons"
```

### 6. Verify
```bash
ssh user@server "systemctl status zomboid-seasons"
```

Check the health endpoint if available.

### 7. Report
- Version/commit deployed
- Service status (running/failed)
- Any warnings from startup logs
