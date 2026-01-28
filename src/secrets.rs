use anyhow::{anyhow, Context, Result};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
}

pub struct EnvStore;

impl SecretStore for EnvStore {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok().filter(|v| !v.is_empty())
    }
}

/// A simple key=value file store.
///
/// Intended default path for system installs: `/etc/acip/secrets.env`.
pub struct EnvFileStore {
    map: HashMap<String, String>,
}

impl EnvFileStore {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        ensure_secure_dotenv(&path)?;

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed reading dotenv file: {}", path.display()))?;

        let mut map = HashMap::new();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Extremely small parser: KEY=VALUE, without quoting/escapes.
            // For more advanced formats, switch to dotenvy parsing.
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                if !k.is_empty() && !v.is_empty() {
                    map.insert(k.to_string(), v.to_string());
                }
            }
        }

        Ok(Self { map })
    }
}

impl SecretStore for EnvFileStore {
    fn get(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned().filter(|v| !v.is_empty())
    }
}

pub struct CompositeStore {
    stores: Vec<Box<dyn SecretStore>>,
}

impl CompositeStore {
    pub fn new(stores: Vec<Box<dyn SecretStore>>) -> Self {
        Self { stores }
    }
}

impl SecretStore for CompositeStore {
    fn get(&self, key: &str) -> Option<String> {
        for s in &self.stores {
            if let Some(v) = s.get(key) {
                return Some(v);
            }
        }
        None
    }
}

/// Enforce that the dotenv file and its parent directory are private.
///
/// Policy:
/// - file must not be readable/writable/executable by group/others (mode & 0o077 == 0)
/// - parent directory must not be accessible by group/others (mode & 0o077 == 0)
///
/// This matches the intent of "700 privs" at the directory level and prevents
/// accidental secret leakage.
pub fn ensure_secure_dotenv(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("dotenv file not found: {}", path.display()));
    }

    let meta = fs::metadata(path)?;

    #[cfg(unix)]
    {
        let mode = meta.mode() & 0o777;
        if (mode & 0o077) != 0 {
            return Err(anyhow!(
                "dotenv file permissions too open (need 600-ish): {} has mode {:o}",
                path.display(),
                mode
            ));
        }

        if let Some(parent) = path.parent() {
            let pmeta = fs::metadata(parent)?;
            let pmode = pmeta.mode() & 0o777;
            if (pmode & 0o077) != 0 {
                return Err(anyhow!(
                    "dotenv parent dir permissions too open (need 700-ish): {} has mode {:o}",
                    parent.display(),
                    pmode
                ));
            }

            // Optional: ensure same owner
            if meta.uid() != pmeta.uid() {
                return Err(anyhow!(
                    "dotenv ownership mismatch: file uid {} vs parent uid {}",
                    meta.uid(),
                    pmeta.uid()
                ));
            }
        }
    }

    Ok(())
}
