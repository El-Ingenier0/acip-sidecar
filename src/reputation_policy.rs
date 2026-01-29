use crate::reputation::ReputationRecord;
use crate::sentry::{Action, Decision, RiskLevel};

#[derive(Debug, Clone)]
pub struct ReputationThresholds {
    pub medium_score: u64,
    pub high_score: u64,
    pub bad_actor_score: u64,
}

impl ReputationThresholds {
    pub fn from_env() -> Self {
        fn get(name: &str, default: u64) -> u64 {
            std::env::var(name)
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(default)
        }

        Self {
            medium_score: get("ACIP_REP_MED", 20),
            high_score: get("ACIP_REP_HIGH", 50),
            bad_actor_score: get("ACIP_REP_BAD", 150),
        }
    }
}

fn bump_risk_level(level: RiskLevel) -> RiskLevel {
    match level {
        RiskLevel::Low => RiskLevel::Medium,
        RiskLevel::Medium => RiskLevel::High,
        RiskLevel::High => RiskLevel::High,
    }
}

/// Apply reputation-based escalation.
///
/// Policy:
/// - Explicit tool authorization may override bad reputation up to `bad_actor_score`.
/// - At/above `bad_actor_score`, tools are always hard-capped off.
pub fn apply_reputation(
    mut decision: Decision,
    allow_tools: bool,
    records: &[ReputationRecord],
    t: &ReputationThresholds,
) -> Decision {
    if records.is_empty() {
        return decision;
    }

    let worst = records
        .iter()
        .max_by_key(|r| (r.risk_score, r.suspected_attack_count))
        .cloned();
    let Some(worst) = worst else { return decision };

    // Always add context reason (non-sensitive).
    decision.reasons.push(format!(
        "source reputation: key={} risk_score={} suspected_attacks={}",
        worst.key, worst.risk_score, worst.suspected_attack_count
    ));

    if worst.risk_score >= t.medium_score {
        decision.risk_level = bump_risk_level(decision.risk_level);
    }

    if worst.risk_score >= t.high_score {
        decision.risk_level = RiskLevel::High;
        // Escalate to needs_review unless already block.
        if !matches!(decision.action, Action::Block) {
            decision.action = Action::NeedsReview;
        }
    }

    // Bad actor cutoff: tools always off, even if explicitly authorized.
    if worst.risk_score >= t.bad_actor_score {
        if decision.tools_allowed {
            decision.tools_allowed = false;
            decision
                .reasons
                .push("tools hard-capped: source classified as bad actor".to_string());
        }
        decision.risk_level = RiskLevel::High;
        if !matches!(decision.action, Action::Block) {
            decision.action = Action::NeedsReview;
        }
        return decision;
    }

    // Below bad-actor cutoff: allow explicit tool auth to win.
    if decision.tools_allowed && !allow_tools {
        decision.tools_allowed = false;
        decision
            .reasons
            .push("tools not authorized by caller".to_string());
    }

    decision
}
