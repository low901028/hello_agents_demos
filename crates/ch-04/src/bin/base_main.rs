use simple_hello_agents_base::llm_client::LLMClient;
use simple_hello_agents_base::tools::{search, ToolExecutor};
use simple_hello_agents_base::react::ReActAgent;
use simple_hello_agents_base::plan_and_solve::PlanAndSolveAgent;
use std::pin::Pin;
use std::future::Future;
use std::sync::Arc;

fn create_search_tool() -> simple_hello_agents_base::tools::ToolFunc {
    Box::new(move |input: String| -> Pin<Box<dyn Future<Output = String> + Send>> {
        Box::pin(async move {
            search(&input).await.unwrap()
        })
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let llm_client = Arc::new(LLMClient::new(None, None, None));

    // ---- ReAct Agent ----
    println!("======= ReAct Agent 演示 =======");
    let mut tool_executor = ToolExecutor::new();
    let search_desc = "一个网页搜索引擎。当你需要回答关于时事、事实以及在你的知识库中找不到的信息时，应使用此工具。";
    tool_executor.register_tool("Search", search_desc, create_search_tool());

    let mut react_agent = ReActAgent::new(llm_client.clone(), tool_executor, 5);
    let question = "华为最新的手机是哪一款？它的主要卖点是什么？";
    if let Some(answer) = react_agent.run(question).await {
        println!("\n最终答案: {}", answer);
    }

    // ---- Plan-and-Solve Agent ----
    println!("\n======= Plan-and-Solve Agent 演示 =======");
    let plan_agent = PlanAndSolveAgent::new(llm_client.clone());
    let question2 = "一个水果店周一卖出了15个苹果。周二卖出的苹果数量是周一的两倍。周三卖出的数量比周二少了5个。请问这三天总共卖出了多少个苹果？";
    plan_agent.run(question2).await;

    // ---- Reflection 演示 ----
    println!("\n======= Reflection 演示 =======");
    let original_code = r#"
def linear_sieve(n: int) -> list[int]:
    if n < 2:
        return []
    is_prime = [True] * (n + 1)
    primes = []
    for i in range(2, n + 1):
        if is_prime[i]:
            primes.append(i)
        for p in primes:
            if i * p > n:
                break
            is_prime[i * p] = False
            if i % p == 0:
                break
    return primes
"#;
    let feedback = "当前代码实现了标准的埃拉托斯特尼筛法...（此处使用你提供的完整反馈）";
    match simple_hello_agents_base::reflection::reflect_and_optimize(&llm_client, original_code, feedback).await {
        Ok(code) => println!("优化后的代码:\n{}", code),
        Err(e) => println!("反思优化出错: {}", e),
    }

    Ok(())
}