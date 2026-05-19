#### 项目概要
HelloAgents-rs 是一个基于 OpenAI 原生 API 构建的生产级多智能体框架， 支持deepseek/gemini/anthropic
集成了
 - 工具响应协议（ToolResponse）、
 - 上下文工程（HistoryManager/TokenCounter）、
 - 会话持久化（SessionStore）、
 - 子代理机制（TaskTool）、
 - 乐观锁（文件编辑）、
 - 熔断器（CircuitBreaker）、
 - Skills 知识外化、
 - TodoWrite 进度管理、
 - DevLog 决策记录、
 - 流式输出（SSE）、
 - 异步生命周期、
 - 可观测性（TraceLogger）、
 - 日志系统（四种范式）、
 - LLM/Agent 基类重构等 16 项核心能力，

为构建复杂智能体应用提供完整的工程化支持。
本项目是基于rust开发实现.
#### [关于core的介绍](core/Core.md)
#### [关于agents的介绍](agents/Agents.md)
#### [关于tools的介绍](tools/Tools.md)
#### [关于context的介绍](context/Context.md)



