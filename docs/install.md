# Installation (Linux / systemd)

This service is intended to run as a small localhost HTTP daemon.

## Paths (convention)

- Config: `/etc/acip/config.toml`
- Secrets: `/etc/acip/secrets.env` (permissions must be private)
- Policies: `/etc/acip/policies.json` (non-secret)

## Prerequisites

- Rust toolchain (for building): `cargo`, `rustc`
- systemd (for service install)

## Build

```bash
git clone https://github.com/El-Ingenier0/moltbot-acip-sidecar.git
cd moltbot-acip-sidecar
cargo build --release
```

Binary will be at:

- `target/release/moltbot-acip-sidecar`

## Create service user/group

```bash
sudo useradd --system --home /nonexistent --shell /usr/sbin/nologin acip || true
sudo groupadd --system acip || true
sudo usermod -a -G acip acip || true
```

## Install files

```bash
sudo install -d -m 0755 /opt/acip
sudo install -m 0755 target/release/moltbot-acip-sidecar /opt/acip/moltbot-acip-sidecar

sudo install -d -m 0755 /etc/acip
sudo install -m 0644 config.example.toml /etc/acip/config.toml

# Optional policies (non-secret)
# sudo install -m 0644 ./policies.example.json /etc/acip/policies.json
```

### Secrets file

Create `/etc/acip/secrets.env` with mode 600 and owned by root (or the service user).

Example:

```bash
sudo install -m 0600 /dev/null /etc/acip/secrets.env
sudoedit /etc/acip/secrets.env
```

Contents (example):

```bash
# Required for live sentry mode:
GEMINI_API_KEY=...
ANTHROPIC_API_KEY=...

# Auth token for callers (optional, but recommended)
ACIP_AUTH_TOKEN=...
```

## Install systemd unit

Copy the unit template:

```bash
sudo install -m 0644 packaging/moltbot-acip-sidecar.service /etc/systemd/system/moltbot-acip-sidecar.service
sudo systemctl daemon-reload
sudo systemctl enable --now moltbot-acip-sidecar
```

Check logs:

```bash
journalctl -u moltbot-acip-sidecar -f
```

## Smoke test

Health:

```bash
curl -sS http://127.0.0.1:18795/health
```

Ingest (token optional depending on config):

```bash
curl -sS \
  -H 'Content-Type: application/json' \
  -H 'X-ACIP-Token: <token>' \
  -d '{
    "source_id":"demo",
    "source_type":"other",
    "content_type":"text/plain",
    "text":"hello world"
  }' \
  http://127.0.0.1:18795/v1/acip/ingest_source | jq
```

## Notes

- `ACIP_SENTRY_MODE=stub` disables model calls; `tools_allowed` stays false.
- For HTML/SVG inputs, tools are hard-capped off by design.
