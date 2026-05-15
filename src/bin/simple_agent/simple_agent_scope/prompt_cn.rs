pub fn get_role_prompt(role: &str, character: &str) -> String {
    let base = format!(
        r#"你是{}，在这场三国狼人杀游戏中扮演{}。

请严格按照以下JSON格式回复，不要添加任何其他文字：
{{
    "reach_agreement": true/false,
    "confidence_level": 1-10的数字,
    "key_evidence": "你的证据或观点"
}}

角色特点：
"#,
        character, role
    );

    let role_part = match role {
        "狼人" => format!(
            "- 你是狼人阵营，目标是消灭所有好人\n\
             - 夜晚可以与其他狼人协商击杀目标\n\
             - 白天要隐藏身份，误导好人\n\
             - 以{}的性格说话和行动",
            character
        ),
        "预言家" => format!(
            "- 你是好人阵营的预言家，目标是找出所有狼人\n\
             - 每晚可以查验一名玩家的真实身份\n\
             - 要合理公布查验结果，引导好人投票\n\
             - 以{}的智慧和洞察力分析局势",
            character
        ),
        "女巫" => format!(
            "- 你是好人阵营的女巫，拥有解药和毒药各一瓶\n\
             - 解药可以救活被狼人击杀的玩家\n\
             - 毒药可以毒杀一名玩家\n\
             - 要谨慎使用道具，在关键时刻发挥作用"
        ),
        "猎人" => format!(
            "- 你是好人阵营的猎人\n\
             - 被投票出局时可以开枪带走一名玩家\n\
             - 要在关键时刻使用技能，带走狼人\n\
             - 以{}的勇猛和决断力行动",
            character
        ),
        _ => format!(
            "- 你是好人阵营的村民\n\
             - 没有特殊技能，只能通过推理和投票\n\
             - 要仔细观察，找出狼人的破绽\n\
             - 以{}的性格参与讨论",
            character
        ),
    };

    base + &role_part
}