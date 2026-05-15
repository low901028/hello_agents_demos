use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
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
use super::msg_hub::MsgHub;

const LLM_TEMPERATURE: f64 = 0.7;

// ==================== Agent ====================

/// ================================================
/// <strong>玩家构建实例</strong>
/// 根据设置的提示词为不同的智能体注入了“游戏角色”和“三国人格”的双重身份
///
/// ================================================
struct DialogAgent {
    name: String,
    role: String,
    system_prompt: String,
    client: Arc<HelloAgentsLLM>,
    history: Vec<Message>,
    rx: Option<broadcast::Receiver<(String, String)>>,
}

impl DialogAgent {
    fn new(name: String, role: String, character: &str, client: Arc<HelloAgentsLLM>) -> Self {
        let system_prompt = get_role_prompt(&role, character);
        Self {
            name,
            role,
            system_prompt,
            client,
            history: Vec::new(),
            rx: None,
        }
    }

    fn bind_hub(&mut self, rx: broadcast::Receiver<(String, String)>) {
        self.rx = Some(rx);
    }

    /// 拉取广播消息（非阻塞）
    fn poll_broadcast(&mut self) {
        if let Some(ref mut rx) = self.rx {
            while let Ok((sender, content)) = rx.try_recv() {
                if sender != self.name {
                    self.history.push(Message::assistant(&sender, &content));
                }
            }
        }
    }

    /// 系统消息（不触发 LLM）
    fn observe(&mut self, content: String) {
        self.history.push(Message::system(content));
    }

    /// 调用 LLM 发言并尝试解析结构化输出
    async fn speak<T: for<'de> serde::Deserialize<'de>>(
        &mut self,
        context: &str,
    ) -> Result<(String, Option<T>)> {
        self.poll_broadcast();

        let mut messages = vec![
            Message {
                role: "system".into(),
                content: self.system_prompt.clone(),
                name: None,
            },
        ];
        messages.extend_from_slice(&self.history);
        messages.push(Message::user(context));

        let response = self.client.think(messages, LLM_TEMPERATURE).await?;

        self.history.push(Message::assistant(&self.name, &response));

        let parsed = parse_json::<T>(&response);
        if parsed.is_none() {
            eprintln!("⚠️ {} 输出解析失败: {:.100}...", self.name, response);
        }

        Ok((response, parsed))
    }
}

/// ==================== 游戏主类 ====================
/// <strong>游戏的主控制器</strong>
/// - 负责维护全局的状态： 玩家存活列表、当前游戏阶段
/// - 推进游戏流程(调用夜晚阶段、白天阶段)
/// - 裁定胜负
///
/// ================================================
pub struct ThreeKingdomsWerewolfGame {
    players: HashMap<String, DialogAgent>,
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
            players: HashMap::new(),                // 玩家
            roles: HashMap::new(),                  // 玩家角色
            moderator: GameModerator::new(),        // 游戏仲裁者
            alive_players: Vec::new(),              // 生存的玩家
            werewolves: Vec::new(),                 // 狼人
            villagers: Vec::new(),                  // 村民
            seer: None,                             // 预言家
            witch: None,                            // 巫婆
            hunter: None,                           // 猎人
            witch_has_antidote: true,               // 巫婆是否有解药
            witch_has_poison: true,                 // 巫婆是否有毒药
            client,                                 // llm client
        }
    }

    /// 生存玩家的姓名
    fn alive_names(&self) -> Vec<&str> {
        self.alive_players.iter().map(|s| s.as_str()).collect()
    }
    /// 消灭所有狼人
    fn non_wolf_targets(&self) -> Vec<String> {
        self.alive_players
            .iter()
            .filter(|p| !self.werewolves.contains(p))
            .cloned()
            .collect()
    }
    /// 生存玩家的角色
    fn get_alive_roles(&self) -> Vec<String> {
        self.alive_players
            .iter()
            .map(|name| self.roles.get(name).cloned().unwrap_or_else(|| "村民".into()))
            .collect()
    }
    /// 提出已阵亡的角色
    fn remove_dead(&mut self, dead: &str) {
        self.alive_players.retain(|p| p != dead);
        self.werewolves.retain(|p| p != dead);
        self.villagers.retain(|p| p != dead);
        if self.seer.as_deref() == Some(dead) { self.seer = None; }
        if self.witch.as_deref() == Some(dead) { self.witch = None; }
        if self.hunter.as_deref() == Some(dead) { self.hunter = None; }
    }
    /// 为指定玩家组创建 MsgHub
    /// 便于不同的玩家组间通信
    fn create_hub_for(&mut self, group: &[String], auto_broadcast: bool) -> (MsgHub, broadcast::Sender<(String, String)>) {
        // 为指定的玩家组，构建message hub
        let hub = MsgHub::new(group.to_vec(), auto_broadcast);
        let tx = hub.sender();
        for name in group {  // 为了指定的玩家组不同的角色绑定到该message hub
            if let Some(agent) = self.players.get_mut(name) {
                let rx = hub.subscribe();
                agent.bind_hub(rx);
            }
        }
        (hub, tx)
    }

    fn broadcast(&self, tx: &broadcast::Sender<(String, String)>, sender: &str, content: &str) {
        let _ = tx.send((sender.to_string(), content.to_string()));
    }

    // ==================== 初始化 ====================
    /// 创建游戏玩家
    async fn create_player(&mut self, role: &str, character: &str) -> Result<()> {
        let name = get_chinese_name(Some(character));
        self.roles.insert(name.clone(), role.to_string());

        let mut agent = DialogAgent::new(name.clone(), role.to_string(), character, self.client.clone());
        let intro = format!(
            "【{}】你在这场三国狼人杀中扮演{}，你的角色是{}。{}",
            name, get_role_desc(role), character, get_role_ability(role)
        );
        agent.observe(self.moderator.announce(&intro));

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
    /// 启动游戏
    async fn setup_game(&mut self, player_count: usize) -> Result<()> {
        println!("🎮 开始设置三国狼人杀游戏...");
        let roles = get_standard_setup(player_count);
        let mut rng = rand::thread_rng();
        let mut characters = vec![
            "刘备", "关羽", "张飞", "诸葛亮", "赵云",
            "曹操", "司马懿", "周瑜", "孙权",
        ];
        characters.shuffle(&mut rng);
        characters.truncate(player_count);

        for (role, character) in roles.iter().zip(characters.iter()) {
            self.create_player(role, character).await?;
        }

        self.moderator.announce(&format!(
            "三国狼人杀游戏开始！参与者：{}",
            format_player_list(&self.alive_names(), None)
        ));
        println!("✅ 游戏设置完成，共{}名玩家", self.alive_players.len());
        Ok(())
    }

    // ==================== 夜晚阶段 ====================
    /// 狼人阶段： 选择击杀目标
    /// 狼人aget
    async fn werewolf_phase(&mut self) -> Option<String> {
        if self.werewolves.is_empty() {
            return None;
        }
        self.moderator.announce("🐺 狼人请睁眼，选择今晚要击杀的目标...");

        let wolves = self.werewolves.clone();
        // 构建狼人间的通信channel
        let (mut hub, tx) = self.create_hub_for(&wolves, true);

        // 狼人间密谋
        // 1、讨论击杀目标
        // 2、彼此交换消息

        // 狼人间讨论
        for _ in 0..MAX_DISCUSSION_ROUND {
            let participants = hub.participants().to_vec();
            for wolf_name in &participants {
                let context = format!(
                    "狼人们，请讨论今晚的击杀目标。存活玩家：{}",
                    format_player_list(&self.alive_names(), None)
                );
                // 向其他狼人交换消息，讨论击杀策略
                if let Some(wolf) = self.players.get_mut(wolf_name) {
                    if let Ok((text, _)) = wolf.speak::<DiscussionModelCN>(&context).await {
                        self.broadcast(&tx, wolf_name, &text);
                    }
                }
            }
        }

        hub.set_auto_broadcast(false);
        // 投票
        let mut votes: HashMap<String, String> = HashMap::new();
        let targets = self.non_wolf_targets();

        // 确定击杀对象
        for wolf_name in &wolves {
            let context = "请选择击杀目标。输出JSON：{\"target\":\"玩家名\",\"kill_strategy\":\"策略\"}";
            if let Some(wolf) = self.players.get_mut(wolf_name) {
                match wolf.speak::<WerewolfKillModelCN>(context).await {
                    Ok((_, Some(parsed))) => { votes.insert(wolf_name.clone(), parsed.target); }
                    _ => {
                        eprintln!("⚠️ {} 投票无效，随机选择目标", wolf_name);
                        if let Some(t) = targets.choose(&mut rand::thread_rng()) {
                            votes.insert(wolf_name.clone(), t.clone());
                        }
                    }
                }
            }
        }

        // 裁决最终的击杀对象
        let (killed, _) = majority_vote_cn(&votes);
        Some(killed)
    }

    /// 预言家阶段：主要是对指定的玩家进行查验
    /// 预言家agent
    async fn seer_phase(&mut self) {
        let seer_name = match &self.seer {
            Some(name) => name.clone(),
            None => return,
        };
        self.moderator.announce("🔮 预言家请睁眼...");

        let context = format!(
            "请选择要查验的玩家。存活玩家：{}。输出JSON：{{\"target\":\"玩家名\",\"check_reason\":\"原因\",\"priority_level\":1-10}}",
            format_player_list(&self.alive_names(), None)
        );

        if let Some(seer) = self.players.get_mut(&seer_name) {
            if let Ok((_, Some(parsed))) = seer.speak::<SeerModelCN>(&context).await {
                let is_wolf = self.roles.get(&parsed.target).map(|r| r == "狼人").unwrap_or(false);
                seer.observe(self.moderator.announce(&format!(
                    "查验结果：{}是{}", parsed.target, if is_wolf { "狼人" } else { "好人" }
                )));
            }
        }
    }

    /// 巫婆阶段
    /// 巫婆agent
    async fn witch_phase(&mut self, killed: Option<String>) -> (Option<String>, Option<String>) {
        let witch_name = match &self.witch {
            Some(name) => name.clone(),
            None => return (killed, None),
        };
        self.moderator.announce("🧙‍♀️ 女巫请睁眼...");

        let info = killed.as_ref()
            .map(|k| format!("今晚{}被狼人击杀", k))
            .unwrap_or_else(|| "今晚平安无事".into());

        if let Some(witch) = self.players.get_mut(&witch_name) {
            witch.observe(self.moderator.announce(&info));
            let context = "是否使用解药/毒药？输出JSON：{\"use_antidote\":bool,\"use_poison\":bool,\"target_name\":\"目标\"}";

            if let Ok((_, Some(parsed))) = witch.speak::<WitchActionModelCN>(context).await {
                let mut saved = None;
                let mut poisoned = None;
                if parsed.use_antidote && self.witch_has_antidote {
                    if let Some(ref k) = killed {
                        saved = Some(k.clone());
                        self.witch_has_antidote = false;
                        witch.observe(self.moderator.announce(&format!("解药救了{}", k)));
                    }
                }
                if parsed.use_poison && self.witch_has_poison {
                    if let Some(ref t) = parsed.target_name {
                        poisoned = Some(t.clone());
                        self.witch_has_poison = false;
                        witch.observe(self.moderator.announce(&format!("毒药毒杀了{}", t)));
                    }
                }
                let final_killed = if saved.is_some() { None } else { killed };
                return (final_killed, poisoned);
            }
        }
        (killed, None)
    }

    // ==================== 白天阶段 ====================
    /// 白天阶段
    async fn day_phase(&mut self, round: usize) -> String {
        self.moderator.day_announcement(round);

        // 构建所有存活玩家间message hub
        let alive = self.alive_players.clone();
        let (mut hub, tx) = self.create_hub_for(&alive, true);
        // 所有存活玩家间自由讨论
        // 讨论agent
        let participants = hub.participants().to_vec();
        for name in &participants {
            let context = format!(
                "请自由讨论。存活玩家：{}。",
                format_player_list(&self.alive_names(), None)
            );
            if let Some(player) = self.players.get_mut(name) {
                if let Ok((text, _)) = player.speak::<DiscussionModelCN>(&context).await {
                    self.broadcast(&tx, name, &text);
                }
            }
        }

        hub.set_auto_broadcast(false);

        // 投票agent
        let mut votes: HashMap<String, String> = HashMap::new();
        for name in &alive {
            let context = format!(
                "请投票淘汰一名玩家。存活玩家：{}。输出JSON：{{\"vote\":\"玩家名\",\"reason\":\"理由\",\"suspicion_level\":1-10}}",
                format_player_list(&self.alive_names(), None)
            );
            if let Some(player) = self.players.get_mut(name) {
                if let Ok((_, Some(parsed))) = player.speak::<VoteModelCN>(&context).await {
                    votes.insert(name.clone(), parsed.vote);
                }
            }
        }

        let (voted, count) = majority_vote_cn(&votes);
        self.moderator.vote_result_announcement(&voted, count);
        voted
    }

    /// 猎人阶段
    /// 猎人agent
    async fn hunter_phase(&mut self, voted_out: &str) -> Option<String> {
        if Some(voted_out) != self.hunter.as_deref() {
            return None;
        }
        let hunter_name = voted_out.to_string();
        self.moderator.announce("🏹 猎人发动技能！");

        let context = format!(
            "你被淘汰，是否开枪？存活玩家：{}。输出JSON：{{\"shoot\":bool,\"target\":\"目标\"}}",
            format_player_list(&self.alive_names(), None)
        );
        // 猎人使用技能淘汰选择玩家
        if let Some(hunter) = self.players.get_mut(&hunter_name) {
            if let Ok((_, Some(parsed))) = hunter.speak::<HunterModelCN>(&context).await {
                if parsed.shoot {
                    if let Some(ref t) = parsed.target {
                        self.moderator.announce(&format!("猎人{}带走了{}", hunter_name, t));
                        return Some(t.clone());
                    }
                }
            }
        }
        None
    }

    // ==================== 主循环 ====================

    pub async fn run_game(&mut self, player_count: usize) -> Result<()> {
        self.setup_game(player_count).await?;

        for round in 1..=MAX_GAME_ROUND {
            println!("\n🌙 === 第{}轮 ===", round);

            // 夜晚
            self.moderator.night_announcement(round);
            let killed = self.werewolf_phase().await;
            self.seer_phase().await;
            let (final_killed, poisoned) = self.witch_phase(killed).await;

            // for dead in [final_killed, poisoned].into_iter().flatten() {
            //     if !dead.is_empty() {
            //         self.remove_dead(&dead);
            //     }
            // }
            let night_deaths: Vec<_> = [final_killed, poisoned].into_iter().flatten().collect();
            self.moderator.death_announcement(&night_deaths);

            if let Some(w) = check_winning_cn(&self.get_alive_roles()) {
                self.moderator.game_over_announcement(&w);
                return Ok(());
            }

            // 白天
            let voted = self.day_phase(round).await;
            let shot = self.hunter_phase(&voted).await;

            self.remove_dead(&voted);
            if let Some(ref s) = shot {
                if !s.is_empty() { self.remove_dead(s); }
            }

            println!("存活：{}", format_player_list(&self.alive_names(), None));

            if let Some(w) = check_winning_cn(&self.get_alive_roles()) {
                self.moderator.game_over_announcement(&w);
                return Ok(());
            }
        }

        println!("达到最大轮次，平局。");
        Ok(())
    }
}