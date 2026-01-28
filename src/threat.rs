use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AttackType {
    PromptInjection,
    ToolCoercion,
    DataExfiltration,
    CredentialTheft,
    Jailbreak,
    SocialEngineering,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThreatAssessment {
    #[serde(default)]
    pub attack_types: Vec<AttackType>,
    #[serde(default)]
    pub indicators: Vec<String>,
    pub threat_score: u8,
}

impl ThreatAssessment {
    pub fn none() -> Self {
        Self {
            attack_types: vec![],
            indicators: vec![],
            threat_score: 0,
        }
    }
}
