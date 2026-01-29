use crate::{model_policy, policy_store, secrets};
use anyhow::Result;
use std::{path::PathBuf, sync::Arc};
use tracing::{info, warn};

/// Build secrets store.
///
/// If `secrets_file` is provided, secrets resolve from: file -> env.
pub fn build_secrets_store(secrets_file: Option<PathBuf>) -> Result<Arc<dyn secrets::SecretStore>> {
    let secrets: Arc<dyn secrets::SecretStore> = if let Some(path) = &secrets_file {
        let file_store = secrets::EnvFileStore::load(path)?;
        Arc::new(secrets::CompositeStore::new(vec![
            Box::new(file_store),
            Box::new(secrets::EnvStore),
        ]))
    } else {
        Arc::new(secrets::EnvStore)
    };

    Ok(secrets)
}

/// Resolve token based on token requirement + secrets store.
pub fn resolve_token(
    token_required: bool,
    secrets: &Arc<dyn secrets::SecretStore>,
    token_env: &str,
) -> Result<Option<String>> {
    if !token_required {
        return Ok(None);
    }

    match secrets.get(token_env) {
        Some(token) if !token.trim().is_empty() => Ok(Some(token)),
        _ => anyhow::bail!(
            "auth token required but missing or empty in secrets store ({})",
            token_env
        ),
    }
}

/// Build policy store.
///
/// If `policies_file` provided, load it. Otherwise derive default policy from env-configured model
/// settings (stored in the same secrets store).
pub fn build_policy_store(
    secrets: &Arc<dyn secrets::SecretStore>,
    policies_file: Option<PathBuf>,
) -> Result<policy_store::PolicyStore> {
    if let Some(policies_path) = &policies_file {
        let pf = policy_store::PoliciesFile::load(policies_path)?;
        return Ok(policy_store::PolicyStore::from_file(pf));
    }

    // Back-compat: derive the default policy from env.
    let mut mp = model_policy::PolicyConfig::default();

    if let Some(p) = secrets.get("ACIP_L1_PROVIDER") {
        if let Some(parsed) = model_policy::Provider::parse(&p) {
            mp.l1.provider = parsed;
        } else {
            warn!("Unknown ACIP_L1_PROVIDER={}; using default", p);
        }
    }
    if let Some(m) = secrets.get("ACIP_L1_MODEL") {
        mp.l1.model = m;
    }

    if let Some(p) = secrets.get("ACIP_L2_PROVIDER") {
        if let Some(parsed) = model_policy::Provider::parse(&p) {
            mp.l2.provider = parsed;
        } else {
            warn!("Unknown ACIP_L2_PROVIDER={}; using default", p);
        }
    }
    if let Some(m) = secrets.get("ACIP_L2_MODEL") {
        mp.l2.model = m;
    }

    info!(
        "model policy: L1={:?}/{}; L2={:?}/{}",
        mp.l1.provider, mp.l1.model, mp.l2.provider, mp.l2.model
    );

    Ok(policy_store::PolicyStore::default_from_env(
        mp.l1.provider.clone(),
        mp.l1.model.clone(),
        mp.l2.provider.clone(),
        mp.l2.model.clone(),
    ))
}
