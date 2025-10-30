use aether_utils::{ApplianceConfig, ApplianceState, HouseholdState};
use axum::{
    routing::{get, post},
    Router,
    Json,
};
use tokio::net::TcpListener;
//use serde::Desrialize;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::Deserialize;


#[tokio::main]
async fn main() {
    println!("[Agent Server] Starting AETHER Agent...");

    // 1. Create our API router
    // This defines all the "routes" our server knows
    let app = Router::new().route("/state", post(handle_state));

    // 2. Define the address to listen on
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("[Agent Server] Listening on http://127.0.0.1:3000");

    // 3. Run the server
    axum::serve(listener, app).await.unwrap();
}

async fn handle_state(
    Json(payload): Json<HouseholdState>
) {
    // For now, just print what we received to prove it works.
    // The {:#?} format is "pretty-print"
    println!("[Agent Server] Received new household state at {}:", payload.timestamp);
    println!("{:#?}", payload.appliances);
    
}