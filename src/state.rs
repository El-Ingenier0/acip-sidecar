use crate::{policy_store::PolicyStore, secrets};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Policy {
    pub head: usize,
    pub tail: usize,
    pub full_if_lte: usize,
}

#[derive(Clone)]
pub struct AppState {
    pub policy: Policy,
    pub secrets: Arc<dyn secrets::SecretStore>,
    pub policies: PolicyStore,
}
