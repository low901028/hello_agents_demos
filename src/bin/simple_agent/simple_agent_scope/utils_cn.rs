use std::collections::HashMap;

pub const MAX_GAME_ROUND: usize = 10;
pub const MAX_DISCUSSION_ROUND: usize = 3;

pub const CHINESE_NAMES: &[&str] = &[
    "刘备", "关羽", "张飞", "诸葛亮", "赵云",
    "曹操", "司马懿", "典韦", "许褚", "夏侯惇",
    "孙权", "周瑜", "陆逊", "甘宁", "太史慈",
    "吕布", "貂蝉", "董卓", "袁绍", "袁术",
];

pub fn get_chinese_name(character: Option<&str>) -> String {
    use rand::Rng;
    if let Some(c) = character {
        if CHINESE_NAMES.contains(&c) {
            return c.to_string();
        }
    }
    let idx = rand::thread_rng().gen_range(0..CHINESE_NAMES.len());
    CHINESE_NAMES[idx].to_string()
}

pub fn format_player_list(players: &[&str], show_roles: Option<&HashMap<String, String>>) -> String {
    if players.is_empty() {
        return "无玩家".into();
    }
    if let Some(roles) = show_roles {
        players
            .iter()
            .map(|name| {
                let role = roles.get(*name).map(|s| s.as_str()).unwrap_or("未知");
                format!("{}({})", name, role)
            })
            .collect::<Vec<_>>()
            .join("、")
    } else {
        players.join("、")
    }
}

pub fn format_player_name_list(players: &[String]) -> String {
    if players.is_empty() {
        "无玩家".into()
    } else {
        players.join("、")
    }
}

pub fn majority_vote_cn(votes: &HashMap<String, String>) -> (String, usize) {
    if votes.is_empty() {
        return ("无人".into(), 0);
    }
    let mut count: HashMap<&str, usize> = HashMap::new();
    for target in votes.values() {
        *count.entry(target.as_str()).or_insert(0) += 1;
    }
    let top = count.into_iter().max_by_key(|&(_, c)| c).unwrap();
    (top.0.to_string(), top.1)
}

pub fn check_winning_cn(alive_roles: &[String]) -> Option<String> {
    let werewolf_count = alive_roles.iter().filter(|r| *r == "狼人").count();
    let villager_count = alive_roles.len() - werewolf_count;

    if werewolf_count == 0 {
        Some("好人阵营胜利！所有狼人已被淘汰！".into())
    } else if werewolf_count >= villager_count {
        Some("狼人阵营胜利！狼人数量已达到或超过好人！".into())
    } else {
        None
    }
}

pub struct GameModerator {
    pub name: String,
    pub game_log: Vec<String>,
}

impl GameModerator {
    pub fn new() -> Self {
        Self {
            name: "游戏主持人".into(),
            game_log: Vec::new(),
        }
    }

    pub fn announce(&mut self, content: &str) -> String {
        self.game_log.push(content.to_string());
        let msg = format!("📢 {}", content);
        println!("{}", msg);
        msg
    }

    pub fn night_announcement(&mut self, round: usize) -> String {
        self.announce(&format!("🌙 第{}夜降临，天黑请闭眼...", round))
    }

    pub fn day_announcement(&mut self, round: usize) -> String {
        self.announce(&format!("☀️ 第{}天天亮了，请大家睁眼...", round))
    }

    pub fn death_announcement(&mut self, dead_players: &[String]) -> String {
        if dead_players.is_empty() {
            self.announce("昨夜平安无事，无人死亡。")
        } else {
            self.announce(&format!("昨夜，{}不幸遇害。", format_player_name_list(dead_players)))
        }
    }

    pub fn vote_result_announcement(&mut self, voted_out: &str, vote_count: usize) -> String {
        self.announce(&format!("投票结果：{}以{}票被淘汰出局。", voted_out, vote_count))
    }

    pub fn game_over_announcement(&mut self, winner: &str) -> String {
        self.announce(&format!("🎉 游戏结束！{}", winner))
    }
}

pub fn calculate_suspicion_score(player_name: &str, game_history: &[HashMap<String, String>]) -> f64 {
    let mut score = 0.0f64;
    for event in game_history {
        if event.get("type").map(|s| s.as_str()) == Some("vote")
            && event.get("target").map(|s| s.as_str()) == Some(player_name)
        {
            score += 0.3;
        } else if event.get("type").map(|s| s.as_str()) == Some("accusation")
            && event.get("target").map(|s| s.as_str()) == Some(player_name)
        {
            score += 0.2;
        } else if event.get("type").map(|s| s.as_str()) == Some("defense")
            && event.get("player").map(|s| s.as_str()) == Some(player_name)
        {
            score -= 0.1;
        }
    }
    score.max(0.0).min(1.0)
}