mod simple_agent;
// ==================== 主函数 ====================
use std::env;
use anyhow::Context;
use dotenvy::dotenv;
use crate::simple_agent::simple_agent_client::HelloAgentsLLM;
use crate::simple_agent::simple_agent_reflaction::ReflectionAgent;

/// ###############################################################
/// Reflection目的
///  - 是为了解决智能体生成初始答案时，它的行动轨迹(执行/推理的步骤)或最终结果，都有可能存在谬论或待改进的地方；
///  - 它属于一种事后(post-hoc)的自我校正循环,审视前面的工作，发现其中的不足，并进行迭代优化
///
/// Reflection的核心思想： 三步循环 执行 -》反思 -》优化
///  - 执行 Execution
///    智能体使用正常的方式(ReAct/Plan-and-Solve)来完成任务，生成一个初步的解决方案或行动轨迹，也就是所谓的"初稿"
///  - 反思 Reflection
///    接着智能体进入反思阶段，通过调用一个独立或带有特殊提示词的LLM，充当“评审员”，它来审视“初稿”，一般会从如下多个维度进行评估
///    - 事实性错误    与常识或已知事实相悖的内容
///    - 逻辑漏洞      推理过程存在不连贯或矛盾之处
///    - 效率问题      是否存在更直接、更简洁高效的路径来完成任务
///    - 遗漏信息      是否忽略了问题中的某些关键约束等
///  在反思环节，会根据评估，生成一段结构化的反馈(feedback)，指出具体的问题所在和改进建议
///  - 优化 Refinement
///   在这个环节，会将“初稿”和“反馈”作为新的上下文，继续调用LLM,要求按照反馈内容对初稿进行校正修改，生成一个相对完善的“修订稿”。
/// 如此循环多次，直到反思阶段不再发现新的问题或达到预设的迭代次数上限
///
/// Reflection价值
/// - 为智能体提供了一个内部纠错回路，并不再完全依赖外部工具的反馈
/// - 将一次性的任务变为一个持续化的过程，提升了复杂任务的最终成功率和答案质量
/// - 为智能体构建了一个临时的“短期记忆”，将“执行-反思-优化”的轨迹形成“经验记录”，
///   这样智能体不但知道了最终答案，也记得自己如何逐步从“有缺陷的初稿”迭代“完善的最终版本”
///
/// Reflection问题
///  - reflection机制就是拿 <strong>成本换质量</strong>
///
/// 一般会在如下的场景中使用reflection机制：
/// - 关键的业务代码生成或技术报告
/// - 科学研究过程中复杂的逻辑推演
/// - 深度分析和规划的决策支持系统
///
/// ReAct、Plan-and-solve、 Reflection的选择
/// - 不确定性太多，并且需要与外部api或资源交互  则优先ReAct(能够根据实时反馈动态调整路径)
/// - 逻辑路径清晰，并侧重内部推理和步骤分解     则优先Plan-and-Solve(能够提供稳定、结构化的执行流程)
/// - 结果的质量和可靠性要求非常高             则优先Reflection(通过迭代优化，将“合格”的答案提升至"优秀")
/// ###############################################################
///
///
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 加载环境变量
    dotenv().ok();

    let model = "deepseek-v4-pro";
    let api_key = env::var("DEEPSEEK_API_KEY").context("请设置 DEEPSEEK_API_KEY 环境变量")?;
    let base_url =
        env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".into());
    let timeout = env::var("LLM_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    let llm_client =
        HelloAgentsLLM::new(Some(&model), Some(&api_key), Some(&base_url), Some(timeout))?;

    let mut agent = ReflectionAgent::new(llm_client, 2);

    let task = "编写一个Python函数，找出1到n之间所有的素数 (prime numbers)。";
    match agent.run(task).await {
        Ok(final_code) => println!("最终代码:\n{}", final_code),
        Err(e) => eprintln!("运行失败: {}", e),
    }

    Ok(())
}