use tokio::sync::{broadcast, mpsc};

use aether_utils::{ApplianceState, HouseholdState, AgentMsg};
use axum::{
    routing::{post,get},
    Router,
    Json,
    response::IntoResponse,
};

use axum::extract::State;
use tokio::net::TcpListener;
use chrono::{DateTime, Utc};
use axum::{
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

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
    //get the latest sim_timestamp field from the database. if is empty, use current time
    let latest_db_sim_timestamp: DateTime<Utc> = match sqlx::query("SELECT sim_timestamp FROM home_state ORDER BY sim_timestamp DESC LIMIT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(row) => row.get::<DateTime<Utc>, _>("sim_timestamp"),
        Err(_) => Utc::now(),
    }; 

    // call handler with owned pool and sender
    handle_state(state.pool.clone(), state.tx.clone(), state.bcast.clone(), payload, latest_db_sim_timestamp).await;
}


// ============================handle_state function=====================================================================
async fn handle_state(pool: Pool<MySql>, tx: mpsc::Sender<AgentMsg>,bcast: broadcast::Sender<String>, payload: HouseholdState, sim_timestamp: DateTime<Utc>) {

    println!("[Agent Server] Received new household state at {}:", payload.timestamp);
    println!("====[HOUSEHOLD STATUS UPDATE ===]\n{:#?}", payload); 

    // insert each appliance's state into the database
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


        sim_timestamp_to_insert = sim_timestamp_to_insert + chrono::Duration::minutes(30);

       
        
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
    // create an async channel for agent messages
    let (tx, rx) = mpsc::channel::<AgentMsg>(100);
    let (bcast_tx, _bcast_rx) = broadcast::channel::<String>(256);

    println!("[AGENT]: Communication channels established!");

    // spawn the agent lookup task and move the receiver + broadcaster into it
    let bcast_for_rules = bcast_tx.clone();
    tokio::spawn(async move {
        rules::initialize_agent_lookup(rx, bcast_for_rules).await;
    });

    // defines all the "routes" using axum
    let pool = connect_to_db().await;
    println!("[DATABASE]: Connected to database.");
    
    let app_state = AppState { pool: pool.clone(), tx: tx.clone() , bcast: bcast_tx.clone()};

    let app = Router::new()
        .route("/state", post(wrapper_handle_state))
        .route("/ws", get(ws_handler))
        .route("/api/predictions", get(predictions_handler))
        .with_state(app_state);
    println!("[Agent Server]: API routes configured.");

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("[Agent Server]: Listening on http://127.0.0.1:3000");

    // Train model and run prediction for a demo month at startup
    match run_prediction_for_month(&pool, 2026, 3).await {
        Ok(prediction) => println!("Prediction: {}", prediction),
        Err(e) => println!("[PREDICTION ERROR]: Failed to run prediction: {}", e),
    }


    // run the server
    axum::serve(listener, app).await.unwrap();
    
}

async fn predictions_handler(State(state): State<AppState>, Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    // parse year and month from query params
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