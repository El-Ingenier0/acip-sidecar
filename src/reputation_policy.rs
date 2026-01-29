use crate::reputation::ReputationRecord;
use crate::sentry::{Action, Decision, RiskLevel};

#[derive(Debug, Clone)]
pub struct ReputationThresholds {
    pub medium_score: u64,
    pub high_score: u64,
    pub bad_actor_score: u64,

    // Adaptive decay knobs
    pub half_life_base_days: f64,
    pub half_life_k: f64,
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
            half_life_base_days: std::env::var("ACIP_REP_HALFLIFE_BASE_DAYS")
                .ok()
                .and_then(|v| v.trim().parse::<f64>().ok())
                .unwrap_or(2.0),
            half_life_k: std::env::var("ACIP_REP_HALFLIFE_K")
                .ok()
                .and_then(|v| v.trim().parse::<f64>().ok())
                .unwrap_or(0.5),
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

fn effective_risk_score(now_unix: u64, r: &ReputationRecord, t: &ReputationThresholds) -> u64 {
    if r.risk_score == 0 {
        return 0;
    }

    let age_secs = now_unix.saturating_sub(r.last_seen_unix);
    let age_days = (age_secs as f64) / 86_400.0;

    // Adaptive half-life: grows with repeated suspected attacks.
    let hl_days = (t.half_life_base_days
        * (1.0 + t.half_life_k * (r.suspected_attack_count as f64)))
        .max(0.1);

    // decay = 0.5^(age/hl)
    let decay = 0.5_f64.powf(age_days / hl_days);
    ((r.risk_score as f64) * decay)
        .round()
        .clamp(0.0, u64::MAX as f64) as u64
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

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut worst_eff: Option<(ReputationRecord, u64)> = None;
    for r in records {
        let eff = effective_risk_score(now_unix, r, t);
        match &worst_eff {
            None => worst_eff = Some((r.clone(), eff)),
            Some((best, best_eff)) => {
                if (eff, r.suspected_attack_count) > (*best_eff, best.suspected_attack_count) {
                    worst_eff = Some((r.clone(), eff));
                }
            }
        }
    }
    let Some((worst, effective_risk)) = worst_eff else {
        return decision;
    };

    // Always add context reason (non-sensitive).
    decision.reasons.push(format!(
        "source reputation: key={} effective_risk={} raw_risk={} suspected_attacks={}",
        worst.key, effective_risk, worst.risk_score, worst.suspected_attack_count
    ));

    if effective_risk >= t.medium_score {
        decision.risk_level = bump_risk_level(decision.risk_level);
    }

    if effective_risk >= t.high_score {
        decision.risk_level = RiskLevel::High;
        // Escalate to needs_review unless already block.
        if !matches!(decision.action, Action::Block) {
            decision.action = Action::NeedsReview;
        }
    }

    // Bad actor cutoff: tools always off, even if explicitly authorized.
    if effective_risk >= t.bad_actor_score {
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
