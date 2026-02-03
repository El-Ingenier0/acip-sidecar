# ACIP Sidecar — Install Mode Decision Tree

Use this to pick the right deployment mode for **acip-sidecar**.

> Goal: smallest attack surface with enough convenience.

## Decision tree

### 1) Are you running in Docker / containers?
- **Yes** → **Mode D: Docker**
- **No** → go to (2)

### 2) Do you have systemd and want the service to survive logouts/reboots?
- **No** → run manually (dev) or use **Mode C: systemd user service** if available
- **Yes** → go to (3)

### 3) Do you need a privileged TCP port (<1024) or other root-only pre-bind setup?
- **Yes** → **Mode B: systemd global (root-drop)**
- **No** → **Mode A: systemd global (direct acip_user)**

### 4) Do you need a Unix domain socket (instead of TCP)?
Any mode can use a Unix socket.
- Set `server.unix_socket = "/run/acip/acip-sidecar.sock"` (systemd global) or a user-writable path (user service).
- When Unix socket is set, TCP host/port are ignored.

---

## Mode A — systemd global (recommended): direct `acip_user`

Use when:
- You have systemd.
- You **do not** need a privileged port.
- You want least privilege and simple operation.

Characteristics:
- systemd starts the process as `acip_user`.
- No “root window.”

Config paths:
- `/etc/acip/config.toml`
- `/etc/acip/secrets.env` (0600)

Unit:
- `packaging/acip-sidecar.service`

---

## Mode B — systemd global (optional): root-drop

Use when:
- You need to bind to a privileged port (<1024), or you must perform a root-only step before serving.

Characteristics:
- systemd starts as root; the process drops privileges to `acip_user` before serving.
- Higher complexity; only use when required.

Unit:
- (planned) `packaging/acip-sidecar.rootdrop.service`

---

## Mode C — systemd user service

Use when:
- You want convenience on a dev machine.
- You accept reduced isolation.

Characteristics:
- Runs as your user.

Unit:
- `packaging/acip-sidecar.user.service`

---

## Mode D — Docker

Use when:
- You want the extraction toolchain (poppler/tesseract) packaged.
- You want predictable runtime dependencies.

Characteristics:
- Runs as a non-root container user by default.
- Expose loopback TCP or bind a unix socket.

Docs:
- `docs/docker.md`
