在hello agents里，除了关键的agent，万物皆可为“tool”.诸如Memory(记忆)、RAG(检索增强)、RL(强化学习)、MCP协议等均被视为“工具”
- base 工具基类
- registry 工具注册机制
- filter 工具过滤器；控制不同类型的 Agent 可以访问哪些工具
- circuit_breaker 工具熔断器; 控制tool调用时 容错机制