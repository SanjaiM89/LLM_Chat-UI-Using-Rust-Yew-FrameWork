use std::{collections::HashMap, env, sync::Arc};

use async_trait::async_trait;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::{error, info};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG")
                .unwrap_or_else(|_| "chat_backend=info,tower_http=info".to_string()),
        )
        .init();

    let state = Arc::new(AppState::new());

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/providers", get(list_providers))
        .route("/api/chat", post(chat_completion))
        .route("/api/settings", get(get_settings))
        .route("/api/settings/agentic", post(set_agentic_mode))
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/agentic/runs", get(list_runs).post(run_agentic_flow))
        .route("/api/agent-templates/openclaw", get(openclaw_template))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    info!("Backend listening at http://0.0.0.0:8080");
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
struct AppState {
    providers: ProviderRegistry,
    settings: Arc<RwLock<Settings>>,
    agents: Arc<RwLock<Vec<AgentDefinition>>>,
    runs: Arc<RwLock<Vec<AgentRun>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            providers: ProviderRegistry::new_from_env(),
            settings: Arc::new(RwLock::new(Settings::default())),
            agents: Arc::new(RwLock::new(seed_agents())),
            runs: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    agentic_mode_enabled: bool,
    default_agent_id: Option<Uuid>,
    crewai_bridge_url: Option<String>,
    agentic_strategy: AgenticStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgenticStrategy {
    Native,
    CrewAiBridge,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            agentic_mode_enabled: false,
            default_agent_id: None,
            crewai_bridge_url: env::var("CREWAI_BRIDGE_URL").ok(),
            agentic_strategy: AgenticStrategy::Native,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentDefinition {
    id: Uuid,
    name: String,
    description: String,
    goal: String,
    system_prompt: String,
    tools: Vec<String>,
    planner_model: String,
    worker_model: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
struct OpenClawTemplate {
    workflow_name: &'static str,
    phases: Vec<&'static str>,
    recommended_tools: Vec<&'static str>,
}

fn seed_agents() -> Vec<AgentDefinition> {
    vec![AgentDefinition {
        id: Uuid::new_v4(),
        name: "Research Analyst".to_string(),
        description: "Collects context, validates claims, and drafts polished answers.".to_string(),
        goal: "Deliver grounded answers with explicit evidence and next actions.".to_string(),
        system_prompt: "Work in steps: plan, gather, synthesize, and report confidence."
            .to_string(),
        tools: vec![
            "web-search".to_string(),
            "connector-hub".to_string(),
            "filesystem".to_string(),
        ],
        planner_model: "openrouter/meta-llama-3.1-70b-instruct".to_string(),
        worker_model: "openai/gpt-4o-mini".to_string(),
        created_at: Utc::now(),
    }]
}

#[derive(Debug, Clone, Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn get_settings(State(state): State<Arc<AppState>>) -> Json<Settings> {
    Json(state.settings.read().await.clone())
}

async fn openclaw_template() -> Json<OpenClawTemplate> {
    Json(OpenClawTemplate {
        workflow_name: "openclaw-inspired-task-graph",
        phases: vec!["plan", "tool-execution", "verification", "final-report"],
        recommended_tools: vec!["terminal", "filesystem", "web-search", "http-client"],
    })
}

#[derive(Debug, Clone, Serialize)]
struct ProviderInfo {
    key: String,
    display_name: String,
    configured: bool,
}

async fn list_providers(State(state): State<Arc<AppState>>) -> Json<Vec<ProviderInfo>> {
    Json(state.providers.list())
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    provider: String,
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    output: String,
    provider: String,
    model: String,
    agentic_mode_enabled: bool,
}

async fn chat_completion(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, ApiError> {
    let settings = state.settings.read().await.clone();
    let base_output = state
        .providers
        .call(&request.provider, &request.model, &request.messages)
        .await?;

    let output = if settings.agentic_mode_enabled {
        format!(
            "[Agentic mode enabled] {base_output}\nTip: use /api/agentic/runs for multi-step runs."
        )
    } else {
        base_output
    };

    Ok(Json(ChatResponse {
        output,
        provider: request.provider,
        model: request.model,
        agentic_mode_enabled: settings.agentic_mode_enabled,
    }))
}

#[derive(Debug, Deserialize)]
struct AgenticToggleRequest {
    enabled: bool,
    default_agent_id: Option<Uuid>,
    crewai_bridge_url: Option<String>,
    agentic_strategy: Option<AgenticStrategy>,
}

async fn set_agentic_mode(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AgenticToggleRequest>,
) -> Json<Settings> {
    let mut settings = state.settings.write().await;
    settings.agentic_mode_enabled = request.enabled;
    if request.default_agent_id.is_some() {
        settings.default_agent_id = request.default_agent_id;
    }
    if let Some(crewai_bridge_url) = request.crewai_bridge_url {
        settings.crewai_bridge_url = Some(crewai_bridge_url);
    }
    if let Some(agentic_strategy) = request.agentic_strategy {
        settings.agentic_strategy = agentic_strategy;
    }
    Json(settings.clone())
}

#[derive(Debug, Deserialize)]
struct CreateAgentRequest {
    name: String,
    description: String,
    goal: String,
    system_prompt: String,
    tools: Vec<String>,
    planner_model: String,
    worker_model: String,
}

async fn list_agents(State(state): State<Arc<AppState>>) -> Json<Vec<AgentDefinition>> {
    Json(state.agents.read().await.clone())
}

async fn create_agent(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateAgentRequest>,
) -> Json<AgentDefinition> {
    let new_agent = AgentDefinition {
        id: Uuid::new_v4(),
        name: request.name,
        description: request.description,
        goal: request.goal,
        system_prompt: request.system_prompt,
        tools: request.tools,
        planner_model: request.planner_model,
        worker_model: request.worker_model,
        created_at: Utc::now(),
    };

    state.agents.write().await.push(new_agent.clone());
    Json(new_agent)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentRun {
    run_id: Uuid,
    agent_id: Uuid,
    objective: String,
    provider: String,
    model: String,
    status: String,
    steps: Vec<AgentStep>,
    final_output: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentStep {
    order: usize,
    title: String,
    tool: String,
    instruction: String,
    output: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CrewAiBridgeRunRequest {
    agent_name: String,
    objective: String,
    goal: String,
    system_prompt: String,
    tools: Vec<String>,
    planner_model: String,
    worker_model: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CrewAiBridgeRunResponse {
    output: String,
}

#[derive(Debug, Deserialize)]
struct AgentRunRequest {
    agent_id: Option<Uuid>,
    objective: String,
    provider: String,
    model: String,
}

async fn list_runs(State(state): State<Arc<AppState>>) -> Json<Vec<AgentRun>> {
    Json(state.runs.read().await.clone())
}

async fn run_agentic_flow(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AgentRunRequest>,
) -> Result<Json<AgentRun>, ApiError> {
    let settings = state.settings.read().await.clone();
    if !settings.agentic_mode_enabled {
        return Err(ApiError::AgenticModeDisabled);
    }

    let target_agent_id = request
        .agent_id
        .or(settings.default_agent_id)
        .ok_or(ApiError::AgentNotSpecified)?;

    let agent = state
        .agents
        .read()
        .await
        .iter()
        .find(|agent| agent.id == target_agent_id)
        .cloned()
        .ok_or(ApiError::AgentNotFound)?;

    let mut steps = plan_steps(&agent, &request.objective);

    if matches!(settings.agentic_strategy, AgenticStrategy::CrewAiBridge) {
        execute_crewai_bridge(&settings, &agent, &request, &mut steps).await?;
    } else {
        execute_steps(
            &state.providers,
            &request.provider,
            &request.model,
            &request.objective,
            &mut steps,
        )
        .await?;
    }

    let final_output = steps
        .iter()
        .map(|step| format!("{}. {} => {}", step.order, step.title, step.output))
        .collect::<Vec<_>>()
        .join("\n");

    let run = AgentRun {
        run_id: Uuid::new_v4(),
        agent_id: agent.id,
        objective: request.objective,
        provider: request.provider,
        model: request.model,
        status: "completed".to_string(),
        steps,
        final_output,
        created_at: Utc::now(),
    };

    state.runs.write().await.push(run.clone());
    Ok(Json(run))
}

fn plan_steps(agent: &AgentDefinition, objective: &str) -> Vec<AgentStep> {
    vec![
        AgentStep {
            order: 1,
            title: "Decompose objective".to_string(),
            tool: "planner".to_string(),
            instruction: format!(
                "Agent '{}' decomposes objective '{}' into concrete subtasks. Goal: {}",
                agent.name, objective, agent.goal
            ),
            output: String::new(),
        },
        AgentStep {
            order: 2,
            title: "Gather data".to_string(),
            tool: agent
                .tools
                .first()
                .cloned()
                .unwrap_or_else(|| "web-search".to_string()),
            instruction: format!(
                "Collect relevant inputs for objective '{}' using declared tools.",
                objective
            ),
            output: String::new(),
        },
        AgentStep {
            order: 3,
            title: "Synthesize and report".to_string(),
            tool: "reporter".to_string(),
            instruction: "Return concise answer, assumptions, and next actions.".to_string(),
            output: String::new(),
        },
    ]
}

async fn execute_crewai_bridge(
    settings: &Settings,
    agent: &AgentDefinition,
    request: &AgentRunRequest,
    steps: &mut [AgentStep],
) -> Result<(), ApiError> {
    let bridge_url = settings
        .crewai_bridge_url
        .clone()
        .ok_or(ApiError::CrewAiBridgeMissing)?;

    let payload = CrewAiBridgeRunRequest {
        agent_name: agent.name.clone(),
        objective: request.objective.clone(),
        goal: agent.goal.clone(),
        system_prompt: agent.system_prompt.clone(),
        tools: agent.tools.clone(),
        planner_model: agent.planner_model.clone(),
        worker_model: agent.worker_model.clone(),
    };

    let response = Client::new()
        .post(bridge_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApiError::ProviderFailure(e.to_string()))?;

    if !response.status().is_success() {
        return Err(ApiError::ProviderFailure(format!(
            "CrewAI bridge failed with status {}",
            response.status()
        )));
    }

    let bridge_response = response
        .json::<CrewAiBridgeRunResponse>()
        .await
        .map_err(|e| ApiError::ProviderFailure(e.to_string()))?;

    for step in steps.iter_mut() {
        step.output = format!(
            "CrewAI handled step {} with aggregated output:\n{}",
            step.order, bridge_response.output
        );
    }

    Ok(())
}

async fn execute_steps(
    providers: &ProviderRegistry,
    provider: &str,
    model: &str,
    objective: &str,
    steps: &mut [AgentStep],
) -> Result<(), ApiError> {
    for step in steps.iter_mut() {
        let prompt = format!(
            "Objective: {objective}\nStep {} ({})\nInstruction: {}",
            step.order, step.tool, step.instruction
        );
        let message = ChatMessage {
            role: "user".to_string(),
            content: prompt,
        };
        let output = providers.call(provider, model, &[message]).await?;
        step.output = output;
    }

    Ok(())
}

#[derive(Debug, Error)]
enum ApiError {
    #[error("provider is not registered")]
    ProviderNotFound,
    #[error("provider failure: {0}")]
    ProviderFailure(String),
    #[error("agentic mode is disabled. enable it in settings first")]
    AgenticModeDisabled,
    #[error("agent id was not provided and no default agent is set")]
    AgentNotSpecified,
    #[error("agent not found")]
    AgentNotFound,
    #[error("CrewAI bridge URL is not configured")]
    CrewAiBridgeMissing,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        error!("API error: {self}");
        let (status, message) = match self {
            Self::ProviderNotFound | Self::AgentNotFound => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            Self::ProviderFailure(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            Self::AgenticModeDisabled | Self::CrewAiBridgeMissing | Self::AgentNotSpecified => {
                (StatusCode::PRECONDITION_FAILED, self.to_string())
            }
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[async_trait]
trait LlmProvider: Send + Sync {
    fn key(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn configured(&self) -> bool;
    async fn completion(&self, model: &str, messages: &[ChatMessage]) -> Result<String, ApiError>;
}

#[derive(Clone)]
struct ProviderRegistry {
    providers: Arc<HashMap<String, Arc<dyn LlmProvider>>>,
}

impl ProviderRegistry {
    fn new_from_env() -> Self {
        let client = Client::new();
        let mut map: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();

        let providers: Vec<Arc<dyn LlmProvider>> = vec![
            Arc::new(HttpProvider::new(
                "openai",
                "OpenAI",
                env::var("OPENAI_API_KEY").ok(),
                "https://api.openai.com/v1/chat/completions",
                client.clone(),
            )),
            Arc::new(HttpProvider::new(
                "openrouter",
                "OpenRouter",
                env::var("OPENROUTER_API_KEY").ok(),
                "https://openrouter.ai/api/v1/chat/completions",
                client.clone(),
            )),
            Arc::new(HttpProvider::new(
                "nvidia",
                "NVIDIA NIM",
                env::var("NVIDIA_API_KEY").ok(),
                "https://integrate.api.nvidia.com/v1/chat/completions",
                client.clone(),
            )),
            Arc::new(HttpProvider::new(
                "gemini",
                "Google Gemini",
                env::var("GEMINI_API_KEY").ok(),
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                client.clone(),
            )),
            Arc::new(HttpProvider::new(
                "claude",
                "Anthropic Claude",
                env::var("ANTHROPIC_API_KEY").ok(),
                "https://api.anthropic.com/v1/messages",
                client.clone(),
            )),
            Arc::new(LocalProvider::new(
                "ollama",
                "Ollama",
                "http://localhost:11434/api/chat",
            )),
            Arc::new(LocalProvider::new(
                "lmstudio",
                "LM Studio",
                "http://localhost:1234/v1/chat/completions",
            )),
            Arc::new(CrewAiBridgeProvider::new()),
        ];

        for provider in providers {
            map.insert(provider.key().to_string(), provider);
        }

        Self {
            providers: Arc::new(map),
        }
    }

    fn list(&self) -> Vec<ProviderInfo> {
        self.providers
            .values()
            .map(|provider| ProviderInfo {
                key: provider.key().to_string(),
                display_name: provider.display_name().to_string(),
                configured: provider.configured(),
            })
            .collect()
    }

    async fn call(
        &self,
        provider_key: &str,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<String, ApiError> {
        let provider = self
            .providers
            .get(provider_key)
            .ok_or(ApiError::ProviderNotFound)?;

        provider.completion(model, messages).await
    }
}

struct HttpProvider {
    key: &'static str,
    display_name: &'static str,
    api_key: Option<String>,
    endpoint: &'static str,
    client: Client,
}

impl HttpProvider {
    fn new(
        key: &'static str,
        display_name: &'static str,
        api_key: Option<String>,
        endpoint: &'static str,
        client: Client,
    ) -> Self {
        Self {
            key,
            display_name,
            api_key,
            endpoint,
            client,
        }
    }
}

#[async_trait]
impl LlmProvider for HttpProvider {
    fn key(&self) -> &'static str {
        self.key
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn configured(&self) -> bool {
        self.api_key.is_some()
    }

    async fn completion(&self, model: &str, messages: &[ChatMessage]) -> Result<String, ApiError> {
        if self.api_key.is_none() {
            let prompt = messages
                .last()
                .map(|m| m.content.as_str())
                .unwrap_or("(no message)");
            return Ok(format!(
                "{} mock response for model '{model}': {prompt}",
                self.display_name
            ));
        }

        let payload = serde_json::json!({
            "model": model,
            "messages": messages,
        });

        let response = self
            .client
            .post(self.endpoint)
            .bearer_auth(
                self.api_key
                    .as_ref()
                    .expect("API key existence checked before calling provider"),
            )
            .json(&payload)
            .send()
            .await
            .map_err(|e| ApiError::ProviderFailure(e.to_string()))?;

        if !response.status().is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "could not decode provider error".to_string());
            return Err(ApiError::ProviderFailure(body));
        }

        let body = response
            .text()
            .await
            .map_err(|e| ApiError::ProviderFailure(e.to_string()))?;

        Ok(format!("{} raw response: {}", self.display_name, body))
    }
}

struct LocalProvider {
    key: &'static str,
    display_name: &'static str,
    endpoint: &'static str,
}

impl LocalProvider {
    fn new(key: &'static str, display_name: &'static str, endpoint: &'static str) -> Self {
        Self {
            key,
            display_name,
            endpoint,
        }
    }
}

#[async_trait]
impl LlmProvider for LocalProvider {
    fn key(&self) -> &'static str {
        self.key
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn configured(&self) -> bool {
        true
    }

    async fn completion(&self, model: &str, messages: &[ChatMessage]) -> Result<String, ApiError> {
        let prompt = messages
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("(no message)");
        Ok(format!(
            "{} endpoint {} would run model '{}' for prompt: {}",
            self.display_name, self.endpoint, model, prompt
        ))
    }
}

struct CrewAiBridgeProvider;

impl CrewAiBridgeProvider {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LlmProvider for CrewAiBridgeProvider {
    fn key(&self) -> &'static str {
        "crewai"
    }

    fn display_name(&self) -> &'static str {
        "CrewAI Bridge"
    }

    fn configured(&self) -> bool {
        true
    }

    async fn completion(&self, model: &str, messages: &[ChatMessage]) -> Result<String, ApiError> {
        let prompt = messages
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or_default();

        let bridge_url = env::var("CREWAI_BRIDGE_URL")
            .unwrap_or_else(|_| "http://localhost:8787/run-crew".to_string());

        Ok(format!(
            "CrewAI bridge placeholder -> POST objective to {bridge_url}. model='{model}', prompt='{prompt}'."
        ))
    }
}
