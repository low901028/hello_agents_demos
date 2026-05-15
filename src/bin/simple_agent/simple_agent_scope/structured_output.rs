use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct DiscussionModelCN {
    pub reach_agreement: bool,
    pub confidence_level: i32,
    #[serde(default)]
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
    #[serde(default)]
    pub target_name: Option<String>,
    #[serde(default)]
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
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub shoot_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WerewolfKillModelCN {
    pub target: String,
    pub kill_strategy: String,
    #[serde(default)]
    pub team_coordination: Option<String>,
}

/// 从 LLM 响应中提取 JSON 并解析为指定类型
pub fn parse_json<T: for<'de> Deserialize<'de>>(text: &str) -> Option<T> {
    let text = text.trim();

    // 1. 尝试直接解析
    if let Ok(val) = serde_json::from_str::<T>(text) {
        return Some(val);
    }

    // 2. 查找 JSON 代码块 (```json ... ```)
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            let json_str = after[..end].trim();
            if let Ok(val) = serde_json::from_str::<T>(json_str) {
                return Some(val);
            }
        }
    }

    // 3. 查找 {} 包裹的 JSON
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            // 注意：start..=end 包含最后一个字符
            let json_str = &text[start..=end];
            if let Ok(val) = serde_json::from_str::<T>(json_str) {
                return Some(val);
            }
        }
    }

    None
}