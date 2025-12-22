use tokio::sync::{broadcast, mpsc};

use aether_utils::{ApplianceState, HouseholdState, AgentMsg};
use axum::{
    routing::{post, get, options},
    Router,
    Json,
    response::IntoResponse,
};

use axum::extract::State;
use tokio::net::TcpListener;
use chrono::{DateTime, Utc, Datelike};
use axum::{
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use serde::Deserialize;
use regex::Regex;
use lazy_static::lazy_static;

mod rules;
mod models;

use sqlx::mysql::{MySqlPool};
use sqlx::{Pool, MySql, Row};

use crate::models::run_prediction_for_month;
use crate::models::run_prediction_for_month_json;
use std::collections::HashMap;
use axum::extract::Query;
use axum::http::StatusCode;

const DB_URL: &str = "mysql://root:20000618MysqlousL@127.0.0.1:3306/aether";
#[derive(Clone)]
struct AppState {
    pool: Pool<MySql>,
    tx: mpsc::Sender<AgentMsg>,
    bcast: broadcast::Sender<String>
}

// =============================wrapper for handle_state to get latest sim_timestamp from db=============================
async fn wrapper_handle_state(State(state): State<AppState>, Json(payload): Json<HouseholdState>) {
    let sim_ts = payload.timestamp;
    handle_state(state.pool.clone(), state.tx.clone(), state.bcast.clone(), payload, sim_ts).await;
}


// ============================handle_state function=====================================================================
async fn handle_state(pool: Pool<MySql>, tx: mpsc::Sender<AgentMsg>,bcast: broadcast::Sender<String>, payload: HouseholdState, sim_timestamp: DateTime<Utc>) {

    println!("[Agent Server] Received new household state at {}:", payload.timestamp);
    println!("====[HOUSEHOLD STATUS UPDATE ===]\n{:#?}", payload); 

    let mut sim_timestamp_to_insert = sim_timestamp;
    for appliance_state in payload.appliances.iter() {
        println!("[Agent Server] Inserting appliance {} with sim_timestamp {}", appliance_state.id, sim_timestamp_to_insert);
        insert_appliance_state(&pool, appliance_state, payload.timestamp, sim_timestamp_to_insert).await;

         // pass the appliance state to agent rules for evaluation in the main loop and increment sim time by 30 mins
        let state_for_agent = AgentMsg{
            state: appliance_state.clone(),
            real_time: payload.timestamp,
            sim_time: sim_timestamp_to_insert,
        };

        if let Err(e) = tx.clone().send(state_for_agent.clone()).await {
            println!("[HANDLE STATE]: Failed to pass data to agent: {}", e);
        }

        // create JSON update
        let bmsg = json!({
            "type": "state",
            "id": state_for_agent.state.id,
            "name": state_for_agent.state.name,
            "watts": state_for_agent.state.watts,
            "is_on": state_for_agent.state.is_on,
            "real_time": state_for_agent.real_time.to_rfc3339(),
            "sim_time": state_for_agent.sim_time.to_rfc3339()
        }).to_string();

        if let Err(e) = bcast.send(bmsg) {
            println!("[BROADCAST] Failed to send realtime update: {}", e);
        }

       
        
    }
}

// =====================================insert_appliance_state function===================================================
async fn insert_appliance_state(pool: &Pool<MySql>, state: &ApplianceState, timestamp: DateTime<Utc>, sim_timestamp: DateTime<Utc>) {
    let query = "INSERT INTO home_state (id, name, watts, is_on, timestamp, sim_timestamp) VALUES (?, ?, ?, ?, ?, ?)";
    match sqlx::query(query)
        .bind(&state.id)
        .bind(&state.name)
        .bind(state.watts)
        .bind(state.is_on)
        .bind(timestamp)
        .bind(sim_timestamp)
        .execute(pool)
        .await
    {
        Ok(_) => println!("[DATABASE] Inserted state for appliance ID: {}", state.id),
        Err(e) => println!("[DATABASE] Failed to insert state for appliance ID: {}: {}", state.id, e),
    }
}

// =====================================connect to  mysql database =======================================================
async fn connect_to_db()-> Pool<MySql> {
    let pool = MySqlPool::connect(DB_URL).await.unwrap();
    pool
}


// ==================================WebSocket handler: upgrade and subscribe to broadcast
async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state.bcast.subscribe()))
}

async fn handle_ws(socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    let (mut sender, mut _receiver) = socket.split();

    loop {
        match rx.recv().await {
            Ok(msg) => {
                // forward JSON text message to client
                if sender.send(Message::Text(msg.into())).await.is_err() {
                    println!("Client disconnected!!");
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                continue;
            }
            Err(_) => break,
        }
    }
}

// ============================= async (tokio) main function ============================================================
#[tokio::main]
async fn main() {
    println!("[Agent Server] Starting AETHER Agent...");

    println!("[AGENT] Calling agent lookup.");
    // async channel for agent messages
    let (tx, rx) = mpsc::channel::<AgentMsg>(100);
    let (bcast_tx, _bcast_rx) = broadcast::channel::<String>(256);

    println!("[AGENT]: Communication channels established!");

    let bcast_for_rules = bcast_tx.clone();
    tokio::spawn(async move {
        rules::initialize_agent_lookup(rx, bcast_for_rules).await;
    });

    let pool = connect_to_db().await;
    println!("[DATABASE]: Connected to database.");
    
    let app_state = AppState { pool: pool.clone(), tx: tx.clone() , bcast: bcast_tx.clone()};

    let app = Router::new()
        .route("/state", post(wrapper_handle_state))
        .route("/ws", get(ws_handler))
        .route("/api/chat/test", get(chat_test_handler))
        .route("/api/predictions", get(predictions_handler))
        .route("/api/chat", options(options_handler).post(chat_handler))
        .with_state(app_state);
    println!("[Agent Server]: API routes configured.");

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("[Agent Server]: Listening on http://127.0.0.1:3000");

    match run_prediction_for_month(&pool, 2026, 3).await {
        Ok(prediction) => println!("Prediction: {}", prediction),
        Err(e) => println!("[PREDICTION ERROR]: Failed to run prediction: {}", e),
    }

    axum::serve(listener, app).await.unwrap();
    
}

async fn predictions_handler(State(state): State<AppState>, Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {

    let year = params.get("year").and_then(|s| s.parse::<i32>().ok()).unwrap_or_else(|| 0);
    let month = params.get("month").and_then(|s| s.parse::<u32>().ok()).unwrap_or_else(|| 0);

    if year <= 0 || month == 0 || month > 12 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid year/month"}))).into_response();
    }

    match run_prediction_for_month_json(&state.pool, year, month).await {
        Ok(val) => ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(val))).into_response(),
        Err(e) => ([("Access-Control-Allow-Origin","*")], (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Prediction failed: {}", e)})))).into_response(),
    }
}

async fn options_handler() -> impl IntoResponse {
    ([
        ("Access-Control-Allow-Origin", "*"),
        ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
        ("Access-Control-Allow-Headers", "Content-Type"),
    ], StatusCode::NO_CONTENT)
}

async fn chat_test_handler() -> impl IntoResponse {
    let resp = json!({"reply": "pong"});
    ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(resp))).into_response()

}

// ====================================ChatRequest struct ===========================================================
#[derive(Deserialize)]
struct ChatRequest {
    query: Option<String>,
    command: Option<String>,
    appliance: Option<String>,
    year: Option<i32>,
    month: Option<u32>,
    n: Option<i32>,
}
// ====================================Define the user's possible intentions ===================================================
enum ChatIntent {
    CountRunning,
    ApplianceStatus,
    TopConsumptionNow,
    TopConsumptionMonth,
    Help,
    Unknown,
}

// ===========================================patterns for intent recognition ===========================================================
lazy_static! {
    static ref COUNT_RUNNING_REGEX: Regex = Regex::new(r"(?i)\b(how many|count|total|active).*?(run|appliances|active)\b").unwrap();
    static ref STATUS_QUERY_REGEX: Regex = Regex::new(r"(?i)\b(is\s+.*?on\?|status\s+of|charger|tv)\b").unwrap();
    static ref TOP_NOW_REGEX: Regex = Regex::new(r"(?i)\b(most|top).*?(watt|power).*?now\b").unwrap();
    static ref TOP_MONTH_REGEX: Regex = Regex::new(r"(?i)\b(most|top|last).*?(month|consumed)\b").unwrap();
    static ref HELP_REGEX: Regex = Regex::new(r"(?i)\b(help|what can you do|commands)\b").unwrap();
}

// ================================================= match user's query into an intent ===================================================
fn classify_intent(q: &str) -> ChatIntent {
    let q_trim = q.trim();
    if COUNT_RUNNING_REGEX.is_match(q_trim) {
        return ChatIntent::CountRunning;
    }
    if q_trim.to_lowercase().starts_with("is ") && q_trim.to_lowercase().ends_with(" on?") {
        return ChatIntent::ApplianceStatus;
    }
    if STATUS_QUERY_REGEX.is_match(q_trim) {
        return ChatIntent::ApplianceStatus;
    }
    if TOP_NOW_REGEX.is_match(q_trim) {
        return ChatIntent::TopConsumptionNow;
    }
    if TOP_MONTH_REGEX.is_match(q_trim) {
        return ChatIntent::TopConsumptionMonth;
    }
    if HELP_REGEX.is_match(q_trim) {
        return ChatIntent::Help;
    }
    ChatIntent::Unknown
}

// ====================================chat handler function =============================================================================
async fn chat_handler(State(state): State<AppState>, Json(payload): Json<ChatRequest>) -> impl IntoResponse {
    let q = payload.query.clone().unwrap_or_default().to_lowercase().trim().to_string();
    println!("[CHAT] Received query: {}", q);

    let latest_ts = match sqlx::query("SELECT sim_timestamp FROM home_state ORDER BY sim_timestamp DESC LIMIT 1").fetch_optional(&state.pool).await {
        Ok(Some(r)) => Some(r.get::<DateTime<Utc>, _>("sim_timestamp")),
        _ => None,
    };

    fn sanitize_name(s: &str) -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .trim()
            .to_string()
    }

    let intent = classify_intent(&q);
    match intent {
        ChatIntent::CountRunning => {
            match sqlx::query(
                "SELECT COUNT(*) AS cnt FROM (
                    SELECT id, is_on 
                    FROM home_state 
                    WHERE (id, sim_timestamp) IN (
                        SELECT id, MAX(sim_timestamp) 
                        FROM home_state 
                        GROUP BY id
                    )
                ) AS latest 
                WHERE is_on = TRUE"
            ).fetch_one(&state.pool).await {
                Ok(r) => {
                    let cnt: i64 = r.get::<i64, _>("cnt");
                    let reply = format!("{} appliance(s) are running now.", cnt);
                    return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": reply})))).into_response();
                }
                Err(e) => {
                    println!("[CHAT ERROR] Count running query failed: {}", e);
                    return ([("Access-Control-Allow-Origin","*")], (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"reply": format!("Query failed: {}", e)})))).into_response();
                }
            }
        }

        ChatIntent::ApplianceStatus => {
            let mut raw_name: Option<String> = None;
            if q.to_lowercase().starts_with("is ") && q.to_lowercase().ends_with(" on?") {
                let r = q.trim_start_matches("is ").trim_end_matches(" on?").trim();
                raw_name = Some(r.to_string());
            } else if q.ends_with('?') && !q.contains(' ') {
                let r = q.trim_end_matches('?').trim();
                raw_name = Some(r.to_string());
            }

            if let Some(raw) = raw_name {
                let name = sanitize_name(&raw);
                println!("[CHAT] Checking appliance: {} (sanitized)", name);
                let pattern = format!("%{}%", name.to_lowercase());
                match sqlx::query("SELECT is_on FROM home_state WHERE LOWER(name) LIKE ? ORDER BY sim_timestamp DESC LIMIT 1").bind(pattern).fetch_optional(&state.pool).await {
                    Ok(Some(r)) => {
                        let is_on: bool = r.get::<bool, _>("is_on");
                        let reply = if is_on { format!("{} is currently ON.", name) } else { format!("{} is currently OFF.", name) };
                        return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": reply})))).into_response();
                    }
                    Ok(None) => return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": format!("No records found for '{}'", name)})))).into_response(),
                    Err(e) => return ([("Access-Control-Allow-Origin","*")], (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"reply": format!("Query failed: {}", e)})))).into_response(),
                }
            }
        }

        ChatIntent::TopConsumptionNow => {
            if let Some(ts) = latest_ts {
                match sqlx::query("SELECT name, watts FROM home_state WHERE sim_timestamp = ? ORDER BY watts DESC LIMIT 1").bind(ts).fetch_optional(&state.pool).await {
                    Ok(Some(r)) => {
                        let name: String = r.get::<String, _>("name");
                        let watts: f64 = r.get::<f64, _>("watts");
                        let reply = format!("Top appliance now is '{}' at {:.2} W", name, watts);
                        return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": reply})))).into_response();
                    }
                    Ok(None) => return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": "."})))).into_response(),
                    Err(e) => return ([("Access-Control-Allow-Origin","*")], (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"reply": format!("Query failed: {}", e)})))).into_response(),
                }
            } else {
                return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": "No recent data available."})))).into_response();
            }
        }

        ChatIntent::TopConsumptionMonth => {
            let now = chrono::Utc::now();
            let mut year = now.year();
            let mut month = now.month();
            if month == 1 {
                month = 12;
                year -= 1;
            } else {
                month -= 1;
            }
            let y = payload.year.unwrap_or(year);
            let m = payload.month.unwrap_or(month);

            println!("[CHAT] Top appliance for target month: {}-{}", y, m);

            let row = sqlx::query("SELECT name, CAST(ROUND(SUM(watts),2) AS DOUBLE) AS total_watts FROM home_state WHERE YEAR(sim_timestamp)=? AND MONTH(sim_timestamp)=? GROUP BY name ORDER BY total_watts DESC LIMIT 1")
                .bind(y)
                .bind(m)
                .fetch_optional(&state.pool)
                .await;

            match row {
                Ok(Some(r)) => {
                    let name: String = r.get::<String, _>("name");
                    let total: f64 = r.get::<f64, _>("total_watts");
                    let reply = format!("Top appliance for {}-{} was '{}' with total {:.2} watts (sum over month).", y, m, name, total);
                    return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": reply})))).into_response();
                }
                Ok(None) => {
                    let fallback = sqlx::query("SELECT YEAR(sim_timestamp) AS yy, MONTH(sim_timestamp) AS mm FROM home_state GROUP BY yy,mm ORDER BY yy DESC, mm DESC LIMIT 1")
                        .fetch_optional(&state.pool)
                        .await;
                    match fallback {
                        Ok(Some(fr)) => {
                            let fy: i32 = fr.get::<i32, _>("yy");
                            let fm: i32 = fr.get::<i32, _>("mm");
                            let row2 = sqlx::query("SELECT name, CAST(ROUND(SUM(watts),2) AS DOUBLE) AS total_watts FROM home_state WHERE YEAR(sim_timestamp)=? AND MONTH(sim_timestamp)=? GROUP BY name ORDER BY total_watts DESC LIMIT 1")
                                .bind(fy)
                                .bind(fm)
                                .fetch_optional(&state.pool)
                                .await;
                            match row2 {
                                Ok(Some(r2)) => {
                                    let name: String = r2.get::<String, _>("name");
                                    let total: f64 = r2.get::<f64, _>("total_watts");
                                    let reply = format!("Showing most recent month {}-{}: Top '{}' with {:.2} watts.", fy, fm, name, total);
                                    return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": reply})))).into_response();
                                }
                                _ => {
                                    let reply = format!("No data found for {}-{} and no fallback month available.", y, m);
                                    return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": reply})))).into_response();
                                }
                            }
                        }
                        _ => {
                            let reply = format!("No data found for {}-{} and no fallback month available.", y, m);
                            return ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": reply})))).into_response();
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("Query failed: {}", e);
                    return ([("Access-Control-Allow-Origin","*")], (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"reply": msg})))).into_response();
                }
            }
        }

        _ => {}
    }

    let help = "Try queries like:\n- 'How many appliances are running now?'\n- 'Is TV on?'\n- 'Most watt consumed appliance last month?'";
    ([("Access-Control-Allow-Origin","*")], (StatusCode::OK, Json(json!({"reply": help})))).into_response()
}