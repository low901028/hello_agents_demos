use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct DiscussionModelCN {
    pub reach_agreement: bool,
    pub confidence_level: i32,
    pub key_evidence: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VoteModelCN {
    pub vote: String,
    pub reason: String,
    pub suspicion_level: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WitchActionModelCN {
    #[serde(default)]
    pub use_antidote: bool,
    #[serde(default)]
    pub use_poison: bool,
    pub target_name: Option<String>,
    pub action_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SeerModelCN {
    pub target: String,
    pub check_reason: String,
    pub priority_level: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HunterModelCN {
    #[serde(default)]
    pub shoot: bool,
    pub target: Option<String>,
    pub shoot_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WerewolfKillModelCN {
    pub target: String,
    pub kill_strategy: String,
    pub team_coordination: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GameAnalysisModelCN {
    #[serde(default)]
    pub suspected_werewolves: Vec<String>,
    #[serde(default)]
    pub trusted_players: Vec<String>,
    #[serde(default)]
    pub key_clues: Vec<String>,
    pub next_strategy: String,
}

/// 解析 LLM 返回的 JSON 为指定结构化类型
pub fn parse_json<T: for<'de> Deserialize<'de>>(text: &str) -> Option<T> {
    // 尝试直接解析
    if let Ok(val) = serde_json::from_str::<T>(text) {
        return Some(val);
    }
    // 尝试从 ```json ... ``` 代码块中提取
    if let Some(start) = text.find("```json") {
        let inner = &text[start + 7..];
        if let Some(end) = inner.find("```") {
            let json_str = &inner[..end].trim();
            if let Ok(val) = serde_json::from_str::<T>(json_str) {
                return Some(val);
            }
        }
    }
    // 尝试找到第一个 { 和最后一个 }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let json_str = &text[start..=end];
            if let Ok(val) = serde_json::from_str::<T>(json_str) {
                return Some(val);
            }
        }
    }
    None
}