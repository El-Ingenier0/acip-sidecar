use acip_sidecar::config;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

/// acipctl — configure and exercise a running ACIP Sidecar.
///
/// Designed to work even when the sidecar runs in Docker: this tool can
/// generate/validate config files and can call the sidecar HTTP API.
#[derive(Debug, Parser)]
#[command(name = "acipctl")]
#[command(version)]
struct Cli {
    /// Base URL for the sidecar (used by commands that call the HTTP API)
    #[arg(long, default_value = "http://127.0.0.1:18795")]
    url: String,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Print/validate/update configuration.
    ///
    /// "Persistent" means editing the config file on disk.
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },

    /// GET /health
    Health,

    /// Ingest a local file via /v1/acip/ingest_source
    IngestFile {
        /// Source id for audit/dedup
        #[arg(long)]
        source_id: String,

        /// Source type (pdf|html|file|other)
        #[arg(long, default_value = "file")]
        source_type: String,

        /// Content-Type (e.g., application/pdf)
        #[arg(long, default_value = "application/octet-stream")]
        content_type: String,

        /// Path to file
        path: PathBuf,

        /// If set, authorizes tools (otherwise tools are hard-gated)
        #[arg(long, default_value_t = false)]
        allow_tools: bool,

        /// Optional policy name to use (header X-ACIP-Policy)
        #[arg(long)]
        policy: Option<String>,
    },

    /// Ingest raw text (reads stdin) via /v1/acip/ingest_source
    IngestText {
        #[arg(long)]
        source_id: String,
        #[arg(long, default_value = "clipboard")]
        source_type: String,
        #[arg(long, default_value = "text/plain")]
        content_type: String,
        #[arg(long, default_value_t = false)]
        allow_tools: bool,
        #[arg(long)]
        policy: Option<String>,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum RestartMode {
    /// systemd global service (default). Runs: sudo systemctl restart acip-sidecar
    System,
    /// systemd user service. Runs: systemctl --user restart acip-sidecar
    User,
    /// Docker compose: print the docker compose restart command (does not run it)
    DockerCompose,
}

#[derive(Debug, Subcommand)]
enum ConfigCmd {
    /// Print a config example to stdout
    Example,

    /// Validate a config file (loads and parses TOML)
    Validate {
        #[arg(long)]
        path: PathBuf,
    },

    /// Show current config file (raw TOML)
    Show {
        #[arg(long)]
        path: PathBuf,
    },

    /// Set a config value and (by default) restart the service.
    ///
    /// Key format: dotted path, e.g. `server.unix_socket` or `policy.head`.
    Set {
        #[arg(long)]
        path: PathBuf,

        /// Dotted key (e.g. server.unix_socket)
        key: String,

        /// Value. Simple auto-typing is supported: true/false, ints, floats, or string.
        value: String,

        /// Restart mode. Default: systemd global.
        #[arg(long, value_enum, default_value_t = RestartMode::System)]
        restart: RestartMode,

        /// For docker-compose restart command output: docker compose file path
        #[arg(long, default_value = "docker-compose.yml")]
        compose_file: String,

        /// For docker-compose restart command output: service name
        #[arg(long, default_value = "acip-sidecar")]
        compose_service: String,

        /// Do not restart; only edit the config file.
        #[arg(long, default_value_t = false)]
        no_restart: bool,
    },

    /// Unset a config value (remove key) and (by default) restart the service.
    ///
    /// Key format: dotted path, e.g. `server.unix_socket`.
    Unset {
        #[arg(long)]
        path: PathBuf,

        /// Dotted key (e.g. server.unix_socket)
        key: String,

        /// Restart mode. Default: systemd global.
        #[arg(long, value_enum, default_value_t = RestartMode::System)]
        restart: RestartMode,

        /// For docker-compose restart command output: docker compose file path
        #[arg(long, default_value = "docker-compose.yml")]
        compose_file: String,

        /// For docker-compose restart command output: service name
        #[arg(long, default_value = "acip-sidecar")]
        compose_service: String,

        /// Do not restart; only edit the config file.
        #[arg(long, default_value_t = false)]
        no_restart: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Config { cmd } => handle_config(cmd)?,

        Cmd::Health => {
            let u = format!("{}/health", cli.url.trim_end_matches('/'));
            let txt = reqwest::blocking::get(&u)
                .with_context(|| format!("GET {u}"))?
                .text()
                .context("read response")?;
            println!("{txt}");
        }

        Cmd::IngestFile {
            source_id,
            source_type,
            content_type,
            path,
            allow_tools,
            policy,
        } => {
            let bytes = fs::read(&path).with_context(|| format!("read {path:?}"))?;
            ingest_bytes(
                &cli.url,
                &source_id,
                &source_type,
                &content_type,
                &bytes,
                allow_tools,
                policy.as_deref(),
            )?;
        }

        Cmd::IngestText {
            source_id,
            source_type,
            content_type,
            allow_tools,
            policy,
        } => {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s).context("read stdin")?;

            // Send as text field; sidecar also accepts bytes_b64.
            let u = format!("{}/v1/acip/ingest_source", cli.url.trim_end_matches('/'));
            let mut req = reqwest::blocking::Client::new().post(&u);
            if allow_tools {
                req = req.header("X-ACIP-Allow-Tools", "true");
            }
            if let Some(p) = policy {
                req = req.header("X-ACIP-Policy", p);
            }

            let body = serde_json::json!({
              "source_id": source_id,
              "source_type": source_type,
              "content_type": content_type,
              "text": s
            });

            let resp = req.json(&body).send().with_context(|| format!("POST {u}"))?;
            let status = resp.status();
            let v: Value = resp.json().context("parse json")?;
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()));
            if !status.is_success() {
                anyhow::bail!("request failed: {status}");
            }
        }
    }

    Ok(())
}

fn handle_config(cmd: ConfigCmd) -> Result<()> {
    match cmd {
        ConfigCmd::Example => {
            let ex = include_str!("../../config.example.toml");
            print!("{ex}");
            Ok(())
        }
        ConfigCmd::Validate { path } => {
            let _ = config::Config::load(&path).with_context(|| format!("load {path:?}"))?;
            eprintln!("OK: {path:?}");
            Ok(())
        }
        ConfigCmd::Show { path } => {
            let txt = fs::read_to_string(&path).with_context(|| format!("read {path:?}"))?;
            print!("{txt}");
            Ok(())
        }
        ConfigCmd::Set {
            path,
            key,
            value,
            restart,
            compose_file,
            compose_service,
            no_restart,
        } => {
            set_config_value(&path, &key, &value)?;
            if no_restart {
                return Ok(());
            }
            restart_service(restart, &compose_file, &compose_service)
        }
        ConfigCmd::Unset {
            path,
            key,
            restart,
            compose_file,
            compose_service,
            no_restart,
        } => {
            unset_config_value(&path, &key)?;
            if no_restart {
                return Ok(());
            }
            restart_service(restart, &compose_file, &compose_service)
        }
    }
}

fn parse_toml_value(s: &str) -> toml_edit::Item {
    let t = s.trim();
    if matches!(t.to_lowercase().as_str(), "true" | "false") {
        return toml_edit::value(t.eq_ignore_ascii_case("true"));
    }
    if let Ok(i) = t.parse::<i64>() {
        return toml_edit::value(i);
    }
    if let Ok(f) = t.parse::<f64>() {
        return toml_edit::value(f);
    }
    toml_edit::value(t)
}

fn set_config_value(path: &PathBuf, dotted_key: &str, value: &str) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {path:?}"))?;
    let mut doc = raw
        .parse::<toml_edit::DocumentMut>()
        .context("parse toml")?;

    let parts: Vec<&str> = dotted_key.split('.').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        anyhow::bail!("invalid key");
    }

    let mut cur: &mut toml_edit::Item = doc.as_item_mut();
    for (i, p) in parts.iter().enumerate() {
        let last = i == parts.len() - 1;
        if last {
            cur[p] = parse_toml_value(value);
        } else {
            // Ensure intermediate tables.
            if !cur[p].is_table() {
                cur[p] = toml_edit::table();
            }
            cur = &mut cur[p];
        }
    }

    // Validate by deserializing with the real config struct.
    let new_txt = doc.to_string();
    let _: config::Config = toml::from_str(&new_txt).context("validate config")?;

    write_atomic(path, &new_txt)?;
    eprintln!("OK: set {dotted_key} in {path:?}");
    Ok(())
}

fn unset_config_value(path: &PathBuf, dotted_key: &str) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {path:?}"))?;
    let mut doc = raw
        .parse::<toml_edit::DocumentMut>()
        .context("parse toml")?;

    let parts: Vec<&str> = dotted_key.split('.').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        anyhow::bail!("invalid key");
    }

    // Walk to parent.
    let mut cur: &mut toml_edit::Item = doc.as_item_mut();
    for p in &parts[..parts.len() - 1] {
        if cur[p].is_none() {
            // Nothing to do.
            return Ok(());
        }
        cur = &mut cur[p];
    }

    if let Some(table) = cur.as_table_mut() {
        table.remove(parts[parts.len() - 1]);
    } else {
        // Parent isn't a table; nothing to remove.
        return Ok(());
    }

    let new_txt = doc.to_string();
    let _: config::Config = toml::from_str(&new_txt).context("validate config")?;

    write_atomic(path, &new_txt)?;
    eprintln!("OK: unset {dotted_key} in {path:?}");
    Ok(())
}

fn write_atomic(path: &PathBuf, contents: &str) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut tf = tempfile::NamedTempFile::new_in(dir).context("create temp file")?;
    tf.write_all(contents.as_bytes()).context("write temp")?;
    tf.flush().ok();
    tf.persist(path).map_err(|e| anyhow::anyhow!(e)).context("persist")?;
    Ok(())
}

fn restart_service(mode: RestartMode, compose_file: &str, compose_service: &str) -> Result<()> {
    match mode {
        RestartMode::System => {
            // Try without sudo first; if it fails, try sudo.
            let status = std::process::Command::new("systemctl")
                .args(["restart", "acip-sidecar"])
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
            let st2 = std::process::Command::new("sudo")
                .args(["systemctl", "restart", "acip-sidecar"])
                .status()
                .context("run sudo systemctl")?;
            if !st2.success() {
                anyhow::bail!("restart failed");
            }
            Ok(())
        }
        RestartMode::User => {
            let st = std::process::Command::new("systemctl")
                .args(["--user", "restart", "acip-sidecar"])
                .status()
                .context("run systemctl --user")?;
            if !st.success() {
                anyhow::bail!("restart failed");
            }
            Ok(())
        }
        RestartMode::DockerCompose => {
            // By design: print the command, do not execute.
            println!(
                "docker compose -f {} restart {}",
                shell_escape(compose_file),
                shell_escape(compose_service)
            );
            Ok(())
        }
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "-._/:".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn ingest_bytes(
    base_url: &str,
    source_id: &str,
    source_type: &str,
    content_type: &str,
    bytes: &[u8],
    allow_tools: bool,
    policy: Option<&str>,
) -> Result<()> {
    let u = format!("{}/v1/acip/ingest_source", base_url.trim_end_matches('/'));

    let mut req = reqwest::blocking::Client::new().post(&u);
    if allow_tools {
        req = req.header("X-ACIP-Allow-Tools", "true");
    }
    if let Some(p) = policy {
        req = req.header("X-ACIP-Policy", p);
    }

    let b64 = B64.encode(bytes);
    let body = serde_json::json!({
      "source_id": source_id,
      "source_type": source_type,
      "content_type": content_type,
      "bytes_b64": b64
    });

    let resp = req.json(&body).send().with_context(|| format!("POST {u}"))?;
    let status = resp.status();
    let v: Value = resp.json().context("parse json")?;
    println!("{}", serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()));
    if !status.is_success() {
        anyhow::bail!("request failed: {status}");
    }
    Ok(())
}
