在hello agents里，除了关键的agent，万物皆可为“tool”.诸如Memory(记忆)、RAG(检索增强)、RL(强化学习)、MCP协议等均被视为“工具”

- base 工具基类
- registry 工具注册机制
- filter 工具过滤器；控制不同类型的 Agent 可以访问哪些工具
- circuit_breaker 工具熔断器; 控制tool调用时 容错机制

### 关于tool的实现

#### step-1 实现trait [Tool](../../hello_agent/tools/base.rs)

impl Tool for XXX {}
[Tool实现样例](../../hello_agent/examples/tool_response_demo.rs)

```rust
impl Tool for DemoCalculatorTool {
    fn name(&self) -> &str {
        //省略代码
    }

    fn description(&self) -> &str { 
        // 省略代码
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        // 省略代码
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        // 省略代码
    }
}
```

#### step-2 构建ToolParameter

> 注意提供的tool name与get_parameters中提供的保持一致；同时确保执行的内容与tool定义中保持一致

``` 
let mut params = HashMap::new();
params.insert("expression".into(), serde_json::json!("2 + 3 * 4"));
```

#### step-3 Tool执行

本质是调用step-1中实现的run

```
let response = tool.run(&params);
```