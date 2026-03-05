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
        .route("/api/settings/agentic", post(set_agentic_mode))
        .route("/api/agents", get(list_agents).post(create_agent))
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
}

impl AppState {
    fn new() -> Self {
        Self {
            providers: ProviderRegistry::new_from_env(),
            settings: Arc::new(RwLock::new(Settings::default())),
            agents: Arc::new(RwLock::new(seed_agents())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    agentic_mode_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            agentic_mode_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentDefinition {
    id: Uuid,
    name: String,
    description: String,
    system_prompt: String,
    tools: Vec<String>,
    model: String,
    created_at: DateTime<Utc>,
}

fn seed_agents() -> Vec<AgentDefinition> {
    vec![AgentDefinition {
        id: Uuid::new_v4(),
        name: "Research Analyst".to_string(),
        description: "Collects and summarizes external context.".to_string(),
        system_prompt: "You are an analyst that finds and cites high-signal information."
            .to_string(),
        tools: vec!["web-search".to_string(), "connector-hub".to_string()],
        model: "openrouter/meta-llama-3.1-70b-instruct".to_string(),
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
    let output = state
        .providers
        .call(&request.provider, &request.model, &request.messages)
        .await?;

    let mut output = output;
    if settings.agentic_mode_enabled {
        output = format!("[CrewAI bridge active] {}", output);
    }

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
}

async fn set_agentic_mode(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AgenticToggleRequest>,
) -> Json<Settings> {
    let mut settings = state.settings.write().await;
    settings.agentic_mode_enabled = request.enabled;
    Json(settings.clone())
}

#[derive(Debug, Deserialize)]
struct CreateAgentRequest {
    name: String,
    description: String,
    system_prompt: String,
    tools: Vec<String>,
    model: String,
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
        system_prompt: request.system_prompt,
        tools: request.tools,
        model: request.model,
        created_at: Utc::now(),
    };

    state.agents.write().await.push(new_agent.clone());
    Json(new_agent)
}

#[derive(Debug, Error)]
enum ApiError {
    #[error("provider is not registered")]
    ProviderNotFound,
    #[error("provider failure: {0}")]
    ProviderFailure(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        error!("API error: {self}");
        let (status, message) = match self {
            Self::ProviderNotFound => (StatusCode::NOT_FOUND, self.to_string()),
            Self::ProviderFailure(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
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
            .bearer_auth(self.api_key.as_ref().unwrap())
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
        Ok(format!(
            "CrewAI bridge invoked with model '{}' and prompt '{}'. Connect a Python CrewAI worker using openclaw-style tools.",
            model, prompt
        ))
    }
}
