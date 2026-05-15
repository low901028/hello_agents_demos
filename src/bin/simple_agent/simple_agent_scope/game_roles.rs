/// 三国狼人杀角色定义
///
/// 使用编译期初始化的静态映射，避免运行时开销

use std::collections::HashMap;

// 用 lazy_static 替代 once_cell（更轻量）
// 实际上直接用函数返回更简单
struct RoleInfo {
    description: &'static str,
    ability: &'static str,
    win_condition: &'static str,
    team: &'static str,
}

fn roles_map() -> HashMap<&'static str, RoleInfo> {
    let mut m = HashMap::new();
    m.insert("狼人", RoleInfo {
        description: "狼人",
        ability: "夜晚可以击杀一名玩家",
        win_condition: "消灭所有好人或与好人数量相等",
        team: "狼人阵营",
    });
    m.insert("预言家", RoleInfo {
        description: "预言家",
        ability: "每晚可以查验一名玩家的身份",
        win_condition: "消灭所有狼人",
        team: "好人阵营",
    });
    m.insert("女巫", RoleInfo {
        description: "女巫",
        ability: "拥有解药和毒药各一瓶，可以救人或杀人",
        win_condition: "消灭所有狼人",
        team: "好人阵营",
    });
    m.insert("猎人", RoleInfo {
        description: "猎人",
        ability: "被投票出局时可以开枪带走一名玩家",
        win_condition: "消灭所有狼人",
        team: "好人阵营",
    });
    m.insert("村民", RoleInfo {
        description: "村民",
        ability: "无特殊技能，依靠推理和投票",
        win_condition: "消灭所有狼人",
        team: "好人阵营",
    });
    m
}

fn traits_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("刘备", "仁德宽厚，善于团结众人，说话温和有礼");
    m.insert("关羽", "忠义刚烈，言辞直接，重情重义");
    m.insert("张飞", "性格豪爽，说话大声直接，容易冲动");
    m.insert("诸葛亮", "智慧超群，分析透彻，言辞谨慎");
    m.insert("赵云", "忠勇双全，话语简洁有力");
    m.insert("曹操", "雄才大略，善于权谋，话语犀利");
    m.insert("司马懿", "深谋远虑，城府极深，言辞含蓄");
    m.insert("周瑜", "才华横溢，略显傲气，分析精准");
    m.insert("孙权", "年轻有为，善于决断，话语果决");
    m
}

pub fn get_role_desc(role: &str) -> String {
    roles_map().get(role).map(|r| r.description.to_string()).unwrap_or_else(|| "未知角色".into())
}

pub fn get_role_ability(role: &str) -> String {
    roles_map().get(role).map(|r| r.ability.to_string()).unwrap_or_else(|| "无特殊技能".into())
}

pub fn get_character_trait(character: &str) -> String {
    traits_map().get(character).map(|s| s.to_string()).unwrap_or_else(|| "性格温和，说话得体".into())
}

pub fn is_werewolf(role: &str) -> bool {
    role == "狼人"
}

pub fn is_villager_team(role: &str) -> bool {
    roles_map().get(role).map(|r| r.team == "好人阵营").unwrap_or(false)
}

pub fn get_standard_setup(player_count: usize) -> Vec<String> {
    let roles = match player_count {
        6 => vec!["狼人", "狼人", "预言家", "女巫", "村民", "村民"],
        8 => vec!["狼人", "狼人", "狼人", "预言家", "女巫", "猎人", "村民", "村民"],
        9 => vec!["狼人", "狼人", "狼人", "预言家", "女巫", "猎人", "守护者", "村民", "村民"],
        _ => {
            // 动态计算
            let werewolf_count = 1.max(player_count / 3);
            let mut roles = vec!["狼人"; werewolf_count];
            let specials = ["预言家", "女巫", "猎人"];
            let special_count = specials.len().min(player_count.saturating_sub(werewolf_count));
            roles.extend_from_slice(&specials[..special_count]);
            let villagers_needed = player_count.saturating_sub(roles.len());
            roles.extend(vec!["村民"; villagers_needed]);
            roles
        }
    };
    roles.into_iter().map(String::from).collect()
}