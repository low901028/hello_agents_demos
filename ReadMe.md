代码整体结构图
```rust
helloagents/
├── core/                           # 核心抽象层（框架契约）
│   ├── traits/                     # Trait 定义（接口隔离）
│   │   ├── agent.rs                # Agent trait（异步 run，依赖 AgentRuntime）
│   │   ├── tool.rs                 # Tool trait（异步 execute，支持展开）
│   │   ├── llm_provider.rs         # LlmProvider trait（异步 chat / chat_stream）
│   │   ├── tool_registry.rs        # ToolRegistry trait（异步执行，注册，熔断）
│   │   ├── history.rs              # HistoryManager trait（历史管理）
│   │   ├── session.rs              # SessionStore trait（会话持久化）
│   │   ├── tool_filter.rs          # ToolFilter trait（工具过滤）
│   │   ├── expandable.rs           # ExpandableTool trait（可展开工具接口）
│   │   └── lifecycle.rs            # LifecycleHook 类型（生命周期钩子）
│   ├── types/                      # 纯数据结构（无行为）
│   │   ├── config.rs               # Config 配置
│   │   ├── event.rs                # AgentEvent / EventType / StreamEvent
│   │   ├── exceptions.rs           # HelloAgentError（带源错误）
│   │   ├── llm_resp_req.rs         # API 请求/响应结构（ChatCompletionRequest 等，保留部分）
│   │   ├── llm_response.rs         # LlmResponse / StreamChunk
│   │   ├── message.rs              # Message / ToolCall / FunctionCall / ToolDefinition / Usage
│   │   ├── response.rs             # ToolResponse / ToolStatus / ErrorInfo（统一工具响应）
│   │   └── session.rs              # SessionData / SessionInfo（会话序列化结构）
│   ├── agent_runtime.rs            # AgentRuntime 聚合结构（依赖注入容器）
│   └── observability.rs            # TraceLogger trait（可观测性接口）
├── infra/                          # 基础设施实现层
│   ├── openai_adapter.rs           # OpenAIAdapter（实现 LlmProvider）
│   ├── tool_registry_impl.rs       # ToolRegistryImpl（实现 ToolRegistry，集成 CircuitBreaker）
│   ├── circuit_breaker.rs          # CircuitBreaker（熔断器）
│   ├── session_store.rs            # SessionStore 实现
│   └── trace_logger.rs             # TraceLogger 实现
├── agents/                         # 智能体实现
│   ├── simple.rs                   # SimpleAgent（对话 + 工具调用）
│   ├── react.rs                    # ReActAgent（推理-行动循环，内置 Thought/Finish）
│   ├── reflection.rs               # ReflectionAgent（反思迭代）
│   └── plan_solve.rs               # PlanSolveAgent（规划-执行分离）
├── tools/                          # 工具系统
│   ├── builtin/                    # 内置工具（实现 Tool trait）
│   │   ├── calculator.rs           # CalculatorTool
│   │   ├── devlog.rs               # DevLogTool
│   │   ├── file.rs                 # ReadTool / WriteTool / EditTool / MultiEditTool
│   │   ├── skill.rs                # SkillTool
│   │   ├── task.rs                 # TaskTool（子代理调用，含 SubagentExecutor trait）
│   │   └── todo_write.rs           # TodoWriteTool
│   ├── filter.rs                   # ToolFilter 实现（ReadOnlyFilter 等）
│   └── error.rs                    # ToolErrorCode 枚举
├── context/                        # 上下文工程
│   ├── history_manager_impl.rs     # HistoryManagerImpl（实现 HistoryManager）
│   ├── token_counter.rs            # TokenCounter
│   ├── builder.rs                  # ContextBuilder（GSSC 流水线）
│   └── truncator.rs                # ObservationTruncator（工具输出截断）
├── skills/                         # 技能系统
│   └── loader.rs                   # SkillLoader（渐进式披露）
└── bin                             # 程序测试（示例）
```