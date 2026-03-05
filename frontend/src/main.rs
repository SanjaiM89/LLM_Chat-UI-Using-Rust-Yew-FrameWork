use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct ChatRequest {
    provider: String,
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct ChatResponse {
    output: String,
    provider: String,
    model: String,
    agentic_mode_enabled: bool,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct AgentDefinition {
    id: String,
    name: String,
    description: String,
    system_prompt: String,
    tools: Vec<String>,
    model: String,
    created_at: String,
}

#[function_component(App)]
fn app() -> Html {
    let prompt = use_state(String::new);
    let provider = use_state(|| "openrouter".to_string());
    let model = use_state(|| "meta-llama/llama-3.1-70b-instruct".to_string());
    let response = use_state(String::new);
    let chat_history = use_state(Vec::<ChatMessage>::new);
    let agentic_mode = use_state(|| false);
    let agents = use_state(Vec::<AgentDefinition>::new);
    let creating_agent = use_state(|| false);

    {
        let agents = agents.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(resp) = Request::get("http://localhost:8080/api/agents")
                    .send()
                    .await
                {
                    if let Ok(data) = resp.json::<Vec<AgentDefinition>>().await {
                        agents.set(data);
                    }
                }
            });
            || ()
        });
    }

    let on_prompt_input = {
        let prompt = prompt.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            prompt.set(input.value());
        })
    };

    let on_provider_change = {
        let provider = provider.clone();
        Callback::from(move |e: Event| {
            let input: HtmlSelectElement = e.target_unchecked_into();
            provider.set(input.value());
        })
    };

    let on_model_change = {
        let model = model.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            model.set(input.value());
        })
    };

    let on_send = {
        let prompt = prompt.clone();
        let provider = provider.clone();
        let model = model.clone();
        let response = response.clone();
        let chat_history = chat_history.clone();
        Callback::from(move |_| {
            let message = ChatMessage {
                role: "user".to_string(),
                content: (*prompt).clone(),
            };

            if message.content.is_empty() {
                return;
            }

            let mut current = (*chat_history).clone();
            current.push(message.clone());
            chat_history.set(current.clone());

            let request = ChatRequest {
                provider: (*provider).clone(),
                model: (*model).clone(),
                messages: current,
            };

            let response = response.clone();
            let prompt = prompt.clone();
            spawn_local(async move {
                let result = Request::post("http://localhost:8080/api/chat")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await;

                match result {
                    Ok(resp) => {
                        if let Ok(data) = resp.json::<ChatResponse>().await {
                            response.set(data.output);
                            prompt.set(String::new());
                        }
                    }
                    Err(_) => {
                        response.set("Backend unavailable. Start backend on :8080".to_string())
                    }
                }
            });
        })
    };

    let on_toggle_agentic = {
        let agentic_mode = agentic_mode.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let enabled = input.checked();
            agentic_mode.set(enabled);
            spawn_local(async move {
                let _ = Request::post("http://localhost:8080/api/settings/agentic")
                    .header("Content-Type", "application/json")
                    .body(serde_json::json!({"enabled": enabled}).to_string())
                    .unwrap()
                    .send()
                    .await;
            });
        })
    };

    let on_create_agent = {
        let creating_agent = creating_agent.clone();
        let agents = agents.clone();
        Callback::from(move |_| {
            creating_agent.set(true);
            let agents = agents.clone();
            let creating_agent = creating_agent.clone();
            spawn_local(async move {
                let payload = serde_json::json!({
                    "name": "Custom Builder Agent",
                    "description": "User-defined agent that follows OpenClaw-like task graph flow.",
                    "system_prompt": "Break work into tools-first steps and summarize progress.",
                    "tools": ["filesystem", "terminal", "web-search"],
                    "model": "openrouter/anthropic/claude-3.5-sonnet"
                });

                let _ = Request::post("http://localhost:8080/api/agents")
                    .header("Content-Type", "application/json")
                    .body(payload.to_string())
                    .unwrap()
                    .send()
                    .await;

                if let Ok(resp) = Request::get("http://localhost:8080/api/agents")
                    .send()
                    .await
                {
                    if let Ok(data) = resp.json::<Vec<AgentDefinition>>().await {
                        agents.set(data);
                    }
                }
                creating_agent.set(false);
            });
        })
    };

    html! {
        <>
            <style>
                {r#"
                body { margin: 0; font-family: Inter, sans-serif; background: #1f1f1f; color: #ece8df; }
                .app { display: grid; grid-template-columns: 290px 1fr 320px; min-height: 100vh; }
                .sidebar { background: #171717; border-right: 1px solid #2f2f2f; padding: 20px 14px; }
                .brand { font-size: 40px; font-family: Georgia, serif; margin-bottom: 20px; }
                .nav-item { padding: 9px 8px; color: #c4c1b9; border-radius: 8px; }
                .nav-item:hover { background: #2a2a2a; }
                .content { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 22px; }
                .headline { font-family: Georgia, serif; font-size: 56px; color: #dfd8c8; }
                .chatbox { width: min(760px, 85%); background: #2a2a2a; border: 1px solid #3c3c3c; border-radius: 24px; padding: 16px; }
                textarea { width: 100%; min-height: 90px; background: transparent; color: #f3f2ef; border: none; resize: vertical; font-size: 18px; }
                textarea:focus { outline: none; }
                .controls { display: flex; gap: 10px; margin-top: 10px; }
                button, select, input { background: #383838; border: 1px solid #4d4d4d; color: #f0ece0; border-radius: 10px; padding: 10px 12px; }
                .settings { background: #1a1a1a; border-left: 1px solid #2f2f2f; padding: 20px; }
                .section { margin-bottom: 24px; }
                .response { white-space: pre-wrap; background:#222; border:1px solid #393939; padding:12px; border-radius:12px; margin-top:14px; color:#d9d3c5; }
                .agent-card { background: #252525; border: 1px solid #353535; border-radius: 12px; padding: 12px; margin-bottom: 10px; }
                "#}
            </style>
            <div class="app">
                <aside class="sidebar">
                    <div class="brand">{"Claude"}</div>
                    <div class="nav-item">{"+ New chat"}</div>
                    <div class="nav-item">{"Search"}</div>
                    <div class="nav-item">{"Customize"}</div>
                    <div class="nav-item">{"Chats"}</div>
                    <div class="nav-item">{"Projects"}</div>
                    <div class="nav-item">{"Artifacts"}</div>
                    <div class="nav-item">{"Code"}</div>
                </aside>

                <main class="content">
                    <div class="headline">{"Evening, builder"}</div>
                    <div class="chatbox">
                        <textarea placeholder="How can I help you today?" value={(*prompt).clone()} oninput={on_prompt_input} />
                        <div class="controls">
                            <select onchange={on_provider_change} value={(*provider).clone()}>
                                <option value="openrouter">{"OpenRouter"}</option>
                                <option value="openai">{"OpenAI"}</option>
                                <option value="nvidia">{"NVIDIA"}</option>
                                <option value="gemini">{"Gemini"}</option>
                                <option value="claude">{"Claude"}</option>
                                <option value="ollama">{"Ollama"}</option>
                                <option value="lmstudio">{"LM Studio"}</option>
                                <option value="crewai">{"CrewAI Bridge"}</option>
                            </select>
                            <input value={(*model).clone()} oninput={on_model_change} />
                            <button onclick={on_send}>{"Send"}</button>
                        </div>
                        <div class="response">{(*response).clone()}</div>
                    </div>
                </main>

                <aside class="settings">
                    <div class="section">
                        <h3>{"Settings"}</h3>
                        <label>
                            <input type="checkbox" checked={*agentic_mode} onchange={on_toggle_agentic} />
                            {" Enable built-in agentic mode (CrewAI bridge)"}
                        </label>
                    </div>

                    <div class="section">
                        <h3>{"Agents"}</h3>
                        <button onclick={on_create_agent} disabled={*creating_agent}>
                            { if *creating_agent {"Creating..."} else {"Create new agent"} }
                        </button>
                        <div style="margin-top:10px;">
                            {for agents.iter().map(|agent| html! {
                                <div class="agent-card">
                                    <b>{&agent.name}</b>
                                    <div>{&agent.description}</div>
                                    <small>{format!("Model: {}", &agent.model)}</small>
                                </div>
                            })}
                        </div>
                    </div>
                </aside>
            </div>
        </>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
