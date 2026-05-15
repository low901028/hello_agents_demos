use anyhow::{Context, Result};
use dotenvy::dotenv;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use crate::simple_agent::simple_agent_client::{HelloAgentsLLM, Message};
use super::game_roles::{get_role_ability, get_role_desc, get_standard_setup};
use super::prompt_cn::get_role_prompt;
use super::structured_output::{
    parse_json, DiscussionModelCN, HunterModelCN, SeerModelCN, VoteModelCN,
    WerewolfKillModelCN, WitchActionModelCN,
};
use super::utils_cn::{
    check_winning_cn, format_player_list, format_player_name_list, get_chinese_name,
    majority_vote_cn, GameModerator, MAX_DISCUSSION_ROUND, MAX_GAME_ROUND,
};

const LLM_TEMPERATURE: f64 = 0.7;

struct Agent {
    name: String,
    role: String,
    character: String,
    system_prompt: String,
    client: Arc<HelloAgentsLLM>,
    history: Vec<Message>,
}

impl Agent {
    fn new(name: String, role: String, character: String, client: Arc<HelloAgentsLLM>) -> Self {
        let system_prompt = get_role_prompt(&role, &character);
        Self {
            name,
            role,
            character,
            system_prompt,
            client,
            history: Vec::new(),
        }
    }

    /// 接收一条旁观消息（不触发回复）
    fn observe(&mut self, content: String) {
        self.history.push(Message {
            role: "system".into(),
            content,
            name: Some("游戏主持人".into()),
        });
    }

    /// 调用 LLM 发言，返回 (content, parsed_json_option)
    async fn speak<T: for<'de> serde::Deserialize<'de>>(
        &mut self,
        context: &str,
    ) -> Result<(String, Option<T>)> {
        let mut messages = vec![Message {
            role: "system".into(),
            content: self.system_prompt.clone(),
            name: None,
        }];
        messages.extend(self.history.clone());
        messages.push(Message {
            role: "user".into(),
            content: context.to_string(),
            name: Some("游戏主持人".into()),
        });

        let response = self.client.think(messages, LLM_TEMPERATURE).await?;

        // 将发言加入历史
        self.history.push(Message {
            role: "assistant".into(),
            content: response.clone(),
            name: Some(self.name.clone()),
        });

        let parsed = parse_json::<T>(&response);
        if parsed.is_none() {
            eprintln!("⚠️ {} 的输出解析失败，原始: {}", self.name, &response[..response.len().min(100)]);
        }

        Ok((response, parsed))
    }
}

pub struct ThreeKingdomsWerewolfGame {
    players: HashMap<String, Agent>,
    roles: HashMap<String, String>,
    moderator: GameModerator,
    alive_players: Vec<String>,
    werewolves: Vec<String>,
    villagers: Vec<String>,
    seer: Option<String>,
    witch: Option<String>,
    hunter: Option<String>,
    witch_has_antidote: bool,
    witch_has_poison: bool,
    client: Arc<HelloAgentsLLM>,
}

impl ThreeKingdomsWerewolfGame {
    pub fn new(client: Arc<HelloAgentsLLM>) -> Self {
        Self {
            players: HashMap::new(),
            roles: HashMap::new(),
            moderator: GameModerator::new(),
            alive_players: Vec::new(),
            werewolves: Vec::new(),
            villagers: Vec::new(),
            seer: None,
            witch: None,
            hunter: None,
            witch_has_antidote: true,
            witch_has_poison: true,
            client,
        }
    }

    async fn create_player(&mut self, role: &str, character: &str) -> Result<()> {
        let name = get_chinese_name(Some(character));
        self.roles.insert(name.clone(), role.to_string());

        let mut agent = Agent::new(
            name.clone(),
            role.to_string(),
            character.to_string(),
            self.client.clone(),
        );

        let intro = format!(
            "【{}】你在这场三国狼人杀中扮演{}，你的角色是{}。{}",
            name,
            get_role_desc(role),
            character,
            get_role_ability(role)
        );
        agent.observe(self.moderator.announce(&intro));

        // 分配到对应阵营
        match role {
            "狼人" => self.werewolves.push(name.clone()),
            "预言家" => self.seer = Some(name.clone()),
            "女巫" => self.witch = Some(name.clone()),
            "猎人" => self.hunter = Some(name.clone()),
            _ => self.villagers.push(name.clone()),
        }
        self.alive_players.push(name.clone());
        self.players.insert(name, agent);

        Ok(())
    }

    async fn setup_game(&mut self, player_count: usize) -> Result<()> {
        println!("🎮 开始设置三国狼人杀游戏...");

        let roles = get_standard_setup(player_count);
        let mut rng = rand::thread_rng();
        let mut characters = vec!["刘备", "关羽", "张飞", "诸葛亮", "赵云", "曹操", "司马懿", "周瑜", "孙权"];
        characters.shuffle(&mut rng);
        characters.truncate(player_count);

        for (role, character) in roles.iter().zip(characters.iter()) {
            self.create_player(role, character).await?;
        }

        let names: Vec<&str> = self.alive_players.iter().map(|s| s.as_str()).collect();
        self.moderator.announce(&format!(
            "三国狼人杀游戏开始！参与者：{}",
            format_player_list(&names, None)
        ));

        println!("✅ 游戏设置完成，共{}名玩家", self.alive_players.len());
        Ok(())
    }

    async fn werewolf_phase(&mut self, round: usize) -> Option<String> {
        if self.werewolves.is_empty() {
            return None;
        }

        self.moderator.announce("🐺 狼人请睁眼，选择今晚要击杀的目标...");

        let alive_names: Vec<&str> = self.alive_players.iter().map(|s| s.as_str()).collect();

        // 狼人讨论（每个狼人发言一轮）
        for _ in 0..MAX_DISCUSSION_ROUND {
            for wolf_name in &self.werewolves.clone() {
                let context = format!(
                    "狼人们，请讨论今晚的击杀目标。存活玩家：{}",
                    format_player_list(&alive_names, None)
                );
                if let Some(wolf) = self.players.get_mut(wolf_name) {
                    let _ = wolf.speak::<DiscussionModelCN>(&context).await;
                }
            }
        }

        // 投票击杀
        let mut votes: HashMap<String, String> = HashMap::new();
        for wolf_name in &self.werewolves.clone() {
            let context = "请选择今晚要击杀的目标，输出JSON格式：{\"target\": \"玩家名\", \"kill_strategy\": \"策略\", ...}";
            if let Some(wolf) = self.players.get_mut(wolf_name) {
                match wolf.speak::<WerewolfKillModelCN>(context).await {
                    Ok((_, Some(parsed))) => {
                        votes.insert(wolf_name.clone(), parsed.target);
                    }
                    _ => {
                        // 随机选择目标
                        println!("⚠️ {} 的击杀投票无效，随机选择目标", wolf_name);
                        let valid: Vec<&String> = self.alive_players.iter()
                            .filter(|p| !self.werewolves.contains(p))
                            .collect();
                        if let Some(target) = valid.choose(&mut rand::thread_rng()) {
                            votes.insert(wolf_name.clone(), (*target).clone());
                        }
                    }
                }
            }
        }

        let (killed, _) = majority_vote_cn(&votes);
        Some(killed)
    }

    async fn seer_phase(&mut self) {
        let seer_name = match &self.seer {
            Some(name) => name.clone(),
            None => return,
        };

        self.moderator.announce("🔮 预言家请睁眼，选择要查验的玩家...");

        let alive_names: Vec<&str> = self.alive_players.iter().map(|s| s.as_str()).collect();
        let context = format!(
            "请选择要查验的玩家。存活玩家：{}。输出JSON：{{\"target\": \"玩家名\", \"check_reason\": \"原因\", \"priority_level\": 1-10}}",
            format_player_list(&alive_names, None)
        );

        if let Some(seer) = self.players.get_mut(&seer_name) {
            if let Ok((_, Some(parsed))) = seer.speak::<SeerModelCN>(&context).await {
                let target_role = self.roles.get(&parsed.target).map(|s| s.as_str()).unwrap_or("村民");
                let result = format!(
                    "查验结果：{}是{}",
                    parsed.target,
                    if target_role == "狼人" { "狼人" } else { "好人" }
                );
                seer.observe(self.moderator.announce(&result));
            } else {
                println!("⚠️ 预言家查验失败，跳过此阶段");
            }
        }
    }

    async fn witch_phase(&mut self, killed_player: Option<String>) -> (Option<String>, Option<String>) {
        let witch_name = match &self.witch {
            Some(name) => name.clone(),
            None => return (killed_player, None),
        };

        self.moderator.announce("🧙‍♀️ 女巫请睁眼...");

        let death_info = if let Some(ref k) = killed_player {
            format!("今晚{}被狼人击杀", k)
        } else {
            "今晚平安无事".into()
        };

        if let Some(witch) = self.players.get_mut(&witch_name) {
            witch.observe(self.moderator.announce(&death_info));

            let context = "请决定是否使用解药和毒药。输出JSON：{\"use_antidote\": true/false, \"use_poison\": true/false, \"target_name\": \"目标\", ...}";
            match witch.speak::<WitchActionModelCN>(context).await {
                Ok((_, Some(parsed))) => {
                    let mut saved = None;
                    let mut poisoned = None;

                    if parsed.use_antidote && self.witch_has_antidote {
                        if let Some(ref k) = killed_player {
                            saved = Some(k.clone());
                            self.witch_has_antidote = false;
                            witch.observe(self.moderator.announce(&format!("你使用解药救了{}", k)));
                        }
                    }
                    if parsed.use_poison && self.witch_has_poison {
                        if let Some(ref target) = parsed.target_name {
                            poisoned = Some(target.clone());
                            self.witch_has_poison = false;
                            witch.observe(self.moderator.announce(&format!("你使用毒药毒杀了{}", target)));
                        }
                    }

                    let final_killed = if saved.is_some() { None } else { killed_player };
                    (final_killed, poisoned)
                }
                _ => {
                    println!("⚠️ 女巫行动失败，视为不使用技能");
                    (killed_player, None)
                }
            }
        } else {
            (killed_player, None)
        }
    }

    async fn hunter_phase(&mut self, voted_out: &str) -> Option<String> {
        let hunter_name = match &self.hunter {
            Some(name) => name.clone(),
            None => return None,
        };

        if hunter_name != voted_out {
            return None;
        }

        self.moderator.announce("🏹 猎人发动技能，可以带走一名玩家...");

        let alive_names: Vec<&str> = self.alive_players.iter().map(|s| s.as_str()).collect();
        let context = format!(
            "你被投票出局，是否开枪？存活玩家：{}。输出JSON：{{\"shoot\": true/false, \"target\": \"目标\", ...}}",
            format_player_list(&alive_names, None)
        );

        if let Some(hunter) = self.players.get_mut(&hunter_name) {
            if let Ok((_, Some(parsed))) = hunter.speak::<HunterModelCN>(&context).await {
                if parsed.shoot {
                    if let Some(ref target) = parsed.target {
                        self.moderator.announce(&format!("猎人{}开枪带走了{}", hunter_name, target));
                        return Some(target.clone());
                    }
                }
            } else {
                println!("⚠️ 猎人技能使用失败，视为放弃开枪");
            }
        }
        None
    }

    fn update_alive_players(&mut self, dead_players: &[String]) {
        for dead in dead_players {
            if dead.is_empty() {
                continue;
            }
            self.alive_players.retain(|p| p != dead);
            self.werewolves.retain(|p| p != dead);
            self.villagers.retain(|p| p != dead);
            if self.seer.as_ref() == Some(dead) {
                self.seer = None;
            }
            if self.witch.as_ref() == Some(dead) {
                self.witch = None;
            }
            if self.hunter.as_ref() == Some(dead) {
                self.hunter = None;
            }
        }
    }

    async fn day_phase(&mut self, round: usize) -> String {
        self.moderator.day_announcement(round);

        let alive_names: Vec<&str> = self.alive_players.iter().map(|s| s.as_str()).collect();

        // 自由讨论（每人发言一轮）
        let players_clone = self.alive_players.clone();
        for player_name in &players_clone {
            let context = format!(
                "现在开始自由讨论。存活玩家：{}。请发表你的看法。",
                format_player_list(&alive_names, None)
            );
            if let Some(player) = self.players.get_mut(player_name) {
                let _ = player.speak::<DiscussionModelCN>(&context).await;
            }
        }

        // 投票阶段
        let mut votes: HashMap<String, String> = HashMap::new();
        for player_name in &self.alive_players.clone() {
            let context = format!(
                "请投票选择要淘汰的玩家。存活玩家：{}。输出JSON：{{\"vote\": \"玩家名\", \"reason\": \"理由\", \"suspicion_level\": 1-10}}",
                format_player_list(&alive_names, None)
            );
            if let Some(player) = self.players.get_mut(player_name) {
                match player.speak::<VoteModelCN>(&context).await {
                    Ok((_, Some(parsed))) => {
                        votes.insert(player_name.clone(), parsed.vote);
                    }
                    _ => {
                        println!("⚠️ {} 的投票无效，视为弃票", player_name);
                    }
                }
            }
        }

        let (voted_out, vote_count) = majority_vote_cn(&votes);
        self.moderator.vote_result_announcement(&voted_out, vote_count);
        voted_out
    }

    pub async fn run_game(&mut self, player_count: usize) -> Result<()> {
        self.setup_game(player_count).await?;

        for round in 1..=MAX_GAME_ROUND {
            println!("\n🌙 === 第{}轮游戏开始 ===", round);

            // 夜晚
            self.moderator.night_announcement(round);
            let killed = self.werewolf_phase(round).await;
            self.seer_phase().await;
            let (final_killed, poisoned) = self.witch_phase(killed).await;

            let night_deaths: Vec<String> = vec![final_killed, poisoned].into_iter().flatten().collect();
            let night_death_names: Vec<String> = night_deaths.clone();
            self.update_alive_players(&night_deaths);
            self.moderator.death_announcement(&night_death_names);

            // 检查胜利条件
            let alive_roles: Vec<String> = self.alive_players.iter()
                .map(|name| self.roles.get(name).cloned().unwrap_or_else(|| "村民".into()))
                .collect();
            if let Some(winner) = check_winning_cn(&alive_roles) {
                self.moderator.game_over_announcement(&winner);
                return Ok(());
            }

            // 白天
            let voted_out = self.day_phase(round).await;
            let hunter_shot = self.hunter_phase(&voted_out).await.map_or("".into(), |v|{v.into()});

            let day_deaths: Vec<String> = vec![voted_out, hunter_shot];
            self.update_alive_players(&day_deaths);

            let alive_names: Vec<&str> = self.alive_players.iter().map(|s| s.as_str()).collect();
            println!("第{}轮结束，存活玩家：{}", round, format_player_list(&alive_names, None));

            let alive_roles: Vec<String> = self.alive_players.iter()
                .map(|name| self.roles.get(name).cloned().unwrap_or_else(|| "村民".into()))
                .collect();
            if let Some(winner) = check_winning_cn(&alive_roles) {
                self.moderator.game_over_announcement(&winner);
                return Ok(());
            }
        }

        println!("游戏达到最大轮次，平局。");
        Ok(())
    }
}