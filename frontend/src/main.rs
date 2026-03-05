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
struct ProviderInfo {
    key: String,
    display_name: String,
    configured: bool,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct AgentDefinition {
    id: String,
    name: String,
    description: String,
    goal: String,
    system_prompt: String,
    tools: Vec<String>,
    planner_model: String,
    worker_model: String,
    created_at: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct AgentRun {
    run_id: String,
    objective: String,
    status: String,
    final_output: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Settings {
    agentic_mode_enabled: bool,
    default_agent_id: Option<String>,
    crewai_bridge_url: Option<String>,
    agentic_strategy: AgenticStrategy,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgenticStrategy {
    Native,
    CrewAiBridge,
}

#[function_component(App)]
fn app() -> Html {
    let prompt = use_state(String::new);
    let objective = use_state(String::new);
    let provider = use_state(|| "openrouter".to_string());
    let model = use_state(|| "meta-llama/llama-3.1-70b-instruct".to_string());
    let response = use_state(String::new);
    let chat_history = use_state(Vec::<ChatMessage>::new);
    let providers = use_state(Vec::<ProviderInfo>::new);
    let agents = use_state(Vec::<AgentDefinition>::new);
    let runs = use_state(Vec::<AgentRun>::new);
    let selected_agent_id = use_state(String::new);
    let agentic_mode = use_state(|| false);
    let strategy = use_state(|| AgenticStrategy::Native);

    let new_agent_name = use_state(|| "Custom Builder Agent".to_string());
    let new_agent_goal = use_state(|| "Complete software tasks with verifiable steps".to_string());

    {
        let providers = providers.clone();
        let agents = agents.clone();
        let runs = runs.clone();
        let agentic_mode = agentic_mode.clone();
        let strategy = strategy.clone();
        let selected_agent_id = selected_agent_id.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(resp) = Request::get("http://localhost:8080/api/providers")
                    .send()
                    .await
                {
                    if let Ok(data) = resp.json::<Vec<ProviderInfo>>().await {
                        providers.set(data);
                    }
                }
                if let Ok(resp) = Request::get("http://localhost:8080/api/agents")
                    .send()
                    .await
                {
                    if let Ok(data) = resp.json::<Vec<AgentDefinition>>().await {
                        if let Some(first) = data.first() {
                            selected_agent_id.set(first.id.clone());
                        }
                        agents.set(data);
                    }
                }
                if let Ok(resp) = Request::get("http://localhost:8080/api/agentic/runs")
                    .send()
                    .await
                {
                    if let Ok(data) = resp.json::<Vec<AgentRun>>().await {
                        runs.set(data);
                    }
                }
                if let Ok(resp) = Request::get("http://localhost:8080/api/settings")
                    .send()
                    .await
                {
                    if let Ok(settings) = resp.json::<Settings>().await {
                        agentic_mode.set(settings.agentic_mode_enabled);
                        strategy.set(settings.agentic_strategy);
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

    let on_objective_input = {
        let objective = objective.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            objective.set(input.value());
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

    let on_agent_change = {
        let selected_agent_id = selected_agent_id.clone();
        Callback::from(move |e: Event| {
            let input: HtmlSelectElement = e.target_unchecked_into();
            selected_agent_id.set(input.value());
        })
    };

    let persist_settings = {
        let agentic_mode = agentic_mode.clone();
        let strategy = strategy.clone();
        let selected_agent_id = selected_agent_id.clone();
        Callback::from(move |_| {
            let strategy_value = if matches!(*strategy, AgenticStrategy::CrewAiBridge) {
                "crew_ai_bridge"
            } else {
                "native"
            };
            let default_agent_id = if selected_agent_id.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String((*selected_agent_id).clone())
            };
            let enabled = *agentic_mode;
            spawn_local(async move {
                let _ = Request::post("http://localhost:8080/api/settings/agentic")
                    .header("Content-Type", "application/json")
                    .body(
                        serde_json::json!({
                            "enabled": enabled,
                            "agentic_strategy": strategy_value,
                            "default_agent_id": default_agent_id
                        })
                        .to_string(),
                    )
                    .unwrap()
                    .send()
                    .await;
            });
        })
    };

    let on_toggle_agentic = {
        let agentic_mode = agentic_mode.clone();
        let persist_settings = persist_settings.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            agentic_mode.set(input.checked());
            persist_settings.emit(());
        })
    };

    let on_strategy_change = {
        let strategy = strategy.clone();
        let persist_settings = persist_settings.clone();
        Callback::from(move |e: Event| {
            let input: HtmlSelectElement = e.target_unchecked_into();
            if input.value() == "crew_ai_bridge" {
                strategy.set(AgenticStrategy::CrewAiBridge);
            } else {
                strategy.set(AgenticStrategy::Native);
            }
            persist_settings.emit(());
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
            current.push(message);
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

    let on_run_agentic = {
        let selected_agent_id = selected_agent_id.clone();
        let objective = objective.clone();
        let provider = provider.clone();
        let model = model.clone();
        let response = response.clone();
        let runs = runs.clone();
        Callback::from(move |_| {
            if objective.is_empty() {
                response.set("Please enter an objective for agentic run".to_string());
                return;
            }

            let agent_id_value = if selected_agent_id.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String((*selected_agent_id).clone())
            };

            let payload = serde_json::json!({
                "agent_id": agent_id_value,
                "objective": (*objective).clone(),
                "provider": (*provider).clone(),
                "model": (*model).clone()
            });

            let response = response.clone();
            let runs = runs.clone();
            spawn_local(async move {
                let result = Request::post("http://localhost:8080/api/agentic/runs")
                    .header("Content-Type", "application/json")
                    .body(payload.to_string())
                    .unwrap()
                    .send()
                    .await;

                match result {
                    Ok(resp) => {
                        if let Ok(body) = resp.text().await {
                            response.set(body);
                        }
                        if let Ok(runs_resp) =
                            Request::get("http://localhost:8080/api/agentic/runs")
                                .send()
                                .await
                        {
                            if let Ok(data) = runs_resp.json::<Vec<AgentRun>>().await {
                                runs.set(data);
                            }
                        }
                    }
                    Err(_) => response.set("Failed to run agentic flow".to_string()),
                }
            });
        })
    };

    let on_new_agent_name = {
        let new_agent_name = new_agent_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            new_agent_name.set(input.value());
        })
    };

    let on_new_agent_goal = {
        let new_agent_goal = new_agent_goal.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            new_agent_goal.set(input.value());
        })
    };

    let on_create_agent = {
        let agents = agents.clone();
        let new_agent_name = new_agent_name.clone();
        let new_agent_goal = new_agent_goal.clone();
        Callback::from(move |_| {
            let payload = serde_json::json!({
                "name": (*new_agent_name).clone(),
                "description": "User-defined agent following OpenClaw-style plan -> execute -> verify loop",
                "goal": (*new_agent_goal).clone(),
                "system_prompt": "Break work into tasks, execute with tools, verify, then report",
                "tools": ["terminal", "filesystem", "web-search"],
                "planner_model": "openrouter/meta-llama-3.1-70b-instruct",
                "worker_model": "openai/gpt-4o-mini"
            });

            let agents = agents.clone();
            spawn_local(async move {
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
            });
        })
    };

    html! {
        <>
            <style>
                {r#"
                body { margin: 0; font-family: Inter, sans-serif; background: #1f1f1f; color: #ece8df; }
                .app { display: grid; grid-template-columns: 280px 1fr 360px; min-height: 100vh; }
                .sidebar { background: #171717; border-right: 1px solid #2f2f2f; padding: 20px 14px; }
                .brand { font-size: 36px; font-family: Georgia, serif; margin-bottom: 20px; }
                .content { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 18px; }
                .chatbox { width: min(840px, 88%); background: #2a2a2a; border: 1px solid #3c3c3c; border-radius: 24px; padding: 16px; }
                textarea { width: 100%; min-height: 90px; background: transparent; color: #f3f2ef; border: none; resize: vertical; font-size: 18px; }
                textarea:focus { outline: none; }
                .controls { display: flex; gap: 10px; margin-top: 10px; flex-wrap: wrap; }
                button, select, input { background: #383838; border: 1px solid #4d4d4d; color: #f0ece0; border-radius: 10px; padding: 10px 12px; }
                .settings { background: #1a1a1a; border-left: 1px solid #2f2f2f; padding: 20px; overflow: auto; }
                .section { margin-bottom: 22px; }
                .response { white-space: pre-wrap; background:#222; border:1px solid #393939; padding:12px; border-radius:12px; margin-top:14px; color:#d9d3c5; max-height: 240px; overflow: auto; }
                .agent-card { background: #252525; border: 1px solid #353535; border-radius: 12px; padding: 10px; margin-bottom: 8px; }
                .run-card { background: #1f1f1f; border: 1px solid #333; border-radius: 8px; padding: 8px; margin-top: 8px; font-size: 12px; }
                "#}
            </style>
            <div class="app">
                <aside class="sidebar">
                    <div class="brand">{"Claude"}</div>
                    <div>{"Agentic mode with CrewAI/OpenClaw-inspired workflow"}</div>
                </aside>

                <main class="content">
                    <div class="chatbox">
                        <textarea placeholder="How can I help you today?" value={(*prompt).clone()} oninput={on_prompt_input} />
                        <div class="controls">
                            <select onchange={on_provider_change} value={(*provider).clone()}>
                                {for providers.iter().map(|p| html! {
                                    <option value={p.key.clone()}>{format!("{}{}", p.display_name, if p.configured {""} else {" (mock)"})}</option>
                                })}
                            </select>
                            <input value={(*model).clone()} oninput={on_model_change} />
                            <button onclick={on_send}>{"Send"}</button>
                        </div>
                        <div class="controls">
                            <select onchange={on_agent_change} value={(*selected_agent_id).clone()}>
                                {for agents.iter().map(|agent| html! {<option value={agent.id.clone()}>{agent.name.clone()}</option>})}
                            </select>
                            <input placeholder="Agentic objective" value={(*objective).clone()} oninput={on_objective_input} />
                            <button onclick={on_run_agentic}>{"Run agentic flow"}</button>
                        </div>
                        <div class="response">{(*response).clone()}</div>
                    </div>
                </main>

                <aside class="settings">
                    <div class="section">
                        <h3>{"Settings"}</h3>
                        <label>
                            <input type="checkbox" checked={*agentic_mode} onchange={on_toggle_agentic} />
                            {" Enable built-in agentic mode"}
                        </label>
                        <div class="controls">
                            <select onchange={on_strategy_change}>
                                <option value="native" selected={matches!(*strategy, AgenticStrategy::Native)}>{"Native planner"}</option>
                                <option value="crew_ai_bridge" selected={matches!(*strategy, AgenticStrategy::CrewAiBridge)}>{"CrewAI bridge"}</option>
                            </select>
                            <button onclick={persist_settings.reform(|_| ())}>{"Save settings"}</button>
                        </div>
                    </div>

                    <div class="section">
                        <h3>{"Create Agent"}</h3>
                        <input value={(*new_agent_name).clone()} oninput={on_new_agent_name} />
                        <input value={(*new_agent_goal).clone()} oninput={on_new_agent_goal} />
                        <button onclick={on_create_agent}>{"Create new agent"}</button>
                    </div>

                    <div class="section">
                        <h3>{"Agents"}</h3>
                        {for agents.iter().map(|agent| html! {
                            <div class="agent-card">
                                <b>{&agent.name}</b>
                                <div>{&agent.description}</div>
                                <small>{format!("Planner: {} | Worker: {}", agent.planner_model, agent.worker_model)}</small>
                            </div>
                        })}
                    </div>

                    <div class="section">
                        <h3>{"Recent agentic runs"}</h3>
                        {for runs.iter().rev().take(5).map(|run| html! {
                            <div class="run-card">
                                <b>{&run.status}</b>
                                <div>{&run.objective}</div>
                            </div>
                        })}
                    </div>
                </aside>
            </div>
        </>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
