use std::collections::HashMap;
use once_cell::sync::Lazy;

static ROLES: Lazy<HashMap<&str, (&str, &str, &str, &str)>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("狼人", ("狼人", "夜晚可以击杀一名玩家", "消灭所有好人或与好人数量相等", "狼人阵营"));
    m.insert("预言家", ("预言家", "每晚可以查验一名玩家的身份", "消灭所有狼人", "好人阵营"));
    m.insert("女巫", ("女巫", "拥有解药和毒药各一瓶，可以救人或杀人", "消灭所有狼人", "好人阵营"));
    m.insert("猎人", ("猎人", "被投票出局时可以开枪带走一名玩家", "消灭所有狼人", "好人阵营"));
    m.insert("村民", ("村民", "无特殊技能，依靠推理和投票", "消灭所有狼人", "好人阵营"));
    m.insert("守护者", ("守护者", "每晚可以守护一名玩家免受狼人攻击", "消灭所有狼人", "好人阵营"));
    m
});

static CHARACTER_TRAITS: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
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
});

pub fn get_role_desc(role: &str) -> String {
    ROLES.get(role).map(|r| r.0.to_string()).unwrap_or_else(|| "未知角色".into())
}

pub fn get_role_ability(role: &str) -> String {
    ROLES.get(role).map(|r| r.1.to_string()).unwrap_or_else(|| "无特殊技能".into())
}

pub fn get_character_trait(character: &str) -> String {
    CHARACTER_TRAITS.get(character).map(|s| s.to_string()).unwrap_or_else(|| "性格温和，说话得体".into())
}

pub fn is_werewolf(role: &str) -> bool {
    role == "狼人"
}

pub fn is_villager_team(role: &str) -> bool {
    ROLES.get(role).map(|r| r.3 == "好人阵营").unwrap_or(false)
}

pub fn get_standard_setup(player_count: usize) -> Vec<String> {
    match player_count {
        6 => vec!["狼人", "狼人", "预言家", "女巫", "村民", "村民"]
            .into_iter().map(String::from).collect(),
        8 => vec!["狼人", "狼人", "狼人", "预言家", "女巫", "猎人", "村民", "村民"]
            .into_iter().map(String::from).collect(),
        9 => vec!["狼人", "狼人", "狼人", "预言家", "女巫", "猎人", "守护者", "村民", "村民"]
            .into_iter().map(String::from).collect(),
        _ => {
            let werewolf_count = 1.max(player_count / 3);
            let mut roles = vec!["狼人".to_string(); werewolf_count];
            let mut remaining = player_count - werewolf_count;
            if remaining > 0 {
                roles.push("预言家".into());
                remaining -= 1;
            }
            if remaining > 0 {
                roles.push("女巫".into());
                remaining -= 1;
            }
            if remaining > 0 {
                roles.push("猎人".into());
                remaining -= 1;
            }
            roles.extend(vec!["村民".to_string(); remaining]);
            roles
        }
    }
}