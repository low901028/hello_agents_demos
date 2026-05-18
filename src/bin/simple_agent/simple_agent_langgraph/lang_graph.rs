use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

// ==================== 核心类型 ====================

pub type ThreadId = String;

pub trait StateData: Clone + Debug + Send + Sync + 'static {}
// impl <T> StateData for T where T: Clone + Debug + Send + Sync + 'static{}

#[derive(Debug, Clone)]
pub struct GraphState<S: StateData> {
    pub data: S,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub name: Option<String>,
}

impl Message {
    pub fn new(role: impl Into<String>, content: impl Into<String>, name: Option<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            name,
        }
    }
}

// ==================== 节点 ====================

#[async_trait]
pub trait NodeFn<S: StateData>: Send + Sync {
    async fn execute(&self, state: GraphState<S>) -> Result<GraphState<S>>;
}

pub struct LambdaNode<S: StateData, F> {
    func: F,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: StateData, F, Fut> LambdaNode<S, F>
where
    F: Fn(GraphState<S>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<GraphState<S>>> + Send + 'static,
{
    pub fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<S: StateData, F, Fut> NodeFn<S> for LambdaNode<S, F>
where
    F: Fn(GraphState<S>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<GraphState<S>>> + Send,
{
    async fn execute(&self, state: GraphState<S>) -> Result<GraphState<S>> {
        (self.func)(state).await
    }
}

// ==================== 边 ====================

#[derive(Debug, Clone)]
pub enum Edge {
    Normal(String),
    Conditional(ConditionalEdge),
}

#[derive(Debug, Clone)]
pub struct ConditionalEdge {
    pub router: String,
    pub mapping: HashMap<String, String>,
}

// ==================== 条件路由 ====================

#[async_trait]
pub trait RouterFn<S: StateData>: Send + Sync {
    async fn route(&self, state: &GraphState<S>) -> Result<String>;
}

pub struct LambdaRouter<S: StateData, F> {
    func: F,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: StateData, F, Fut> LambdaRouter<S, F>
where
    F: Fn(&GraphState<S>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<String>> + Send + 'static,
{
    pub fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<S: StateData, F, Fut> RouterFn<S> for LambdaRouter<S, F>
where
    F: Fn(&GraphState<S>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<String>> + Send,
{
    async fn route(&self, state: &GraphState<S>) -> Result<String> {
        (self.func)(state).await
    }
}

// ==================== 检查点 ====================

#[derive(Debug, Clone)]
pub struct Checkpoint<S: StateData> {
    pub state: GraphState<S>,
    pub step: usize,
}

#[derive(Clone)]
pub struct InMemorySaver<S: StateData> {
    checkpoints: Arc<RwLock<HashMap<ThreadId, Vec<Checkpoint<S>>>>>,
}

impl<S: StateData> InMemorySaver<S> {
    pub fn new() -> Self {
        Self {
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn save(&self, thread_id: &ThreadId, checkpoint: Checkpoint<S>) {
        let mut map = self.checkpoints.write().await;
        map.entry(thread_id.clone())
            .or_insert_with(Vec::new)
            .push(checkpoint);
    }

    pub async fn get_latest(&self, thread_id: &ThreadId) -> Option<Checkpoint<S>> {
        let map = self.checkpoints.read().await;
        map.get(thread_id).and_then(|v| v.last().cloned())
    }
}

// ==================== 图构建器 ====================

pub struct StateGraphBuilder<S: StateData> {
    nodes: HashMap<String, Arc<dyn NodeFn<S>>>,
    edges: HashMap<String, Edge>,
    routers: HashMap<String, Arc<dyn RouterFn<S>>>,
    entry_point: Option<String>,
}

impl<S: StateData> StateGraphBuilder<S> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            routers: HashMap::new(),
            entry_point: None,
        }
    }

    pub fn add_node<F, Fut>(&mut self, name: impl Into<String>, func: F) -> &mut Self
    where
        F: Fn(GraphState<S>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<GraphState<S>>> + Send + 'static,
    {
        self.nodes
            .insert(name.into(), Arc::new(LambdaNode::new(func)));
        self
    }

    pub fn set_entry_point(&mut self, name: impl Into<String>) -> &mut Self {
        self.entry_point = Some(name.into());
        self
    }

    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) -> &mut Self {
        self.edges
            .insert(from.into(), Edge::Normal(to.into()));
        self
    }

    pub fn add_conditional_edges<F, Fut>(
        &mut self,
        source: impl Into<String>,
        router_func: F,
        mapping: HashMap<String, String>,
    ) -> &mut Self
    where
        F: Fn(&GraphState<S>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<String>> + Send + 'static,
    {
        let router_name = format!("router_{}", Uuid::new_v4());
        self.routers
            .insert(router_name.clone(), Arc::new(LambdaRouter::new(router_func)));
        self.edges.insert(
            source.into(),
            Edge::Conditional(ConditionalEdge {
                router: router_name,
                mapping,
            }),
        );
        self
    }

    pub fn compile(self, checkpointer: InMemorySaver<S>) -> CompiledGraph<S> {
        CompiledGraph {
            nodes: self.nodes,
            edges: self.edges,
            routers: self.routers,
            entry_point: self.entry_point.unwrap_or_else(|| "start".into()),
            checkpointer,
        }
    }
}

// ==================== 编译后的可执行图 ====================

#[derive(Clone)]
pub struct CompiledGraph<S: StateData> {
    nodes: HashMap<String, Arc<dyn NodeFn<S>>>,
    edges: HashMap<String, Edge>,
    routers: HashMap<String, Arc<dyn RouterFn<S>>>,
    entry_point: String,
    checkpointer: InMemorySaver<S>,
}

#[derive(Debug, Clone)]
pub enum StreamEvent<S: StateData> {
    NodeStart(String),
    NodeEnd(String, GraphState<S>),
    CheckpointSaved(ThreadId, usize),
    Complete(GraphState<S>),
}

impl<S: StateData> CompiledGraph<S> {
    /// 非流式调用：使用 `Option` 包裹状态，彻底消除 move 后使用
    pub async fn invoke(
        &self,
        thread_id: &ThreadId,
        initial_state: GraphState<S>,
    ) -> Result<GraphState<S>> {
        let mut state_wrapper = Some(initial_state);
        let mut current_node = self.entry_point.clone();
        let mut step = 0usize;

        loop {
            step += 1;

            let current_state = match state_wrapper.take() {
                Some(s) => s,
                None => return Err(anyhow::anyhow!("状态丢失")),
            };

            let node = self.nodes.get(&current_node)
                .ok_or_else(|| anyhow::anyhow!("节点不存在: {}", current_node))?;

            let new_state = node.execute(current_state).await?;

            self.checkpointer.save(thread_id, Checkpoint {
                state: new_state.clone(),
                step,
            }).await;

            let edge = self.edges.get(&current_node);
            let next = match edge {
                Some(Edge::Normal(next)) => Some(next.clone()),
                Some(Edge::Conditional(cond)) => {
                    match self.routers.get(&cond.router) {
                        Some(router) => {
                            let route = router.route(&new_state).await?;
                            cond.mapping.get(&route).cloned()
                        }
                        None => {
                            eprintln!("⚠️ 路由 '{}' 不存在", cond.router);
                            None
                        }
                    }
                }
                None => None,
            };

            state_wrapper = Some(new_state);

            match next {
                Some(next_node) => current_node = next_node,
                None => break,
            }
        }

        state_wrapper.ok_or_else(|| anyhow::anyhow!("图执行结束但状态丢失"))
    }

    /// 流式调用：使用 `Option` 包裹状态，彻底消除 move 后使用
    pub async fn stream(
        &self,
        thread_id: &ThreadId,
        initial_state: GraphState<S>,
    ) -> Result<mpsc::UnboundedReceiver<StreamEvent<S>>> {
        let (tx, rx) = mpsc::unbounded_channel();

        let nodes = self.nodes.clone();
        let edges = self.edges.clone();
        let routers = self.routers.clone();
        let entry_point = self.entry_point.clone();
        let checkpointer = self.checkpointer.clone();
        let tid = thread_id.clone();

        tokio::spawn(async move {
            let mut state_wrapper = Some(initial_state);
            let mut current_node = entry_point;
            let mut step = 0usize;

            loop {
                step += 1;
                let _ = tx.send(StreamEvent::NodeStart(current_node.clone()));

                // 安全取出状态
                let current_state = match state_wrapper.take() {
                    Some(s) => s,
                    None => break,
                };

                let node = match nodes.get(&current_node) {
                    Some(n) => n.clone(),
                    None => {
                        let _ = tx.send(StreamEvent::Complete(current_state));
                        break;
                    }
                };

                // 保存快照用于错误恢复
                let prev_state = current_state.clone();

                // 执行节点
                let new_state = match node.execute(current_state).await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("节点执行错误: {}", e);
                        let _ = tx.send(StreamEvent::Complete(prev_state));
                        break;
                    }
                };

                let _ = tx.send(StreamEvent::NodeEnd(current_node.clone(), new_state.clone()));

                checkpointer
                    .save(&tid, Checkpoint {
                        state: new_state.clone(),
                        step,
                    })
                    .await;
                let _ = tx.send(StreamEvent::CheckpointSaved(tid.clone(), step));

                // ✅ 修正：直接 match Option<&Arc<RouterFn>>
                let edge = edges.get(&current_node);
                let next = match edge {
                    Some(Edge::Normal(next)) => Some(next.clone()),
                    Some(Edge::Conditional(cond)) => {
                        match routers.get(&cond.router) {
                            Some(router) => {
                                match router.route(&new_state).await {
                                    Ok(route) => cond.mapping.get(&route).cloned(),
                                    Err(e) => {
                                        eprintln!("⚠️ 路由执行失败: {}", e);
                                        None
                                    }
                                }
                            }
                            None => {
                                eprintln!("⚠️ 路由 '{}' 不存在", cond.router);
                                None
                            }
                        }
                    }
                    None => None,
                };

                // 放回新状态
                state_wrapper = Some(new_state);

                match next {
                    Some(next_node) => current_node = next_node,
                    None => break,
                }
            }

            // 发送最终状态
            if let Some(final_state) = state_wrapper.take() {
                let _ = tx.send(StreamEvent::Complete(final_state));
            }
        });

        Ok(rx)
    }
}