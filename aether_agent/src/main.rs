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

use std::env; // To get the connection string

// Import sqlx
use sqlx::mysql::{MySqlPool, MySqlRow};
use sqlx::{Pool, MySql, Row};

const DB_URL: &str = "mysql://root:20000618MysqlousL@127.0.0.1:3306/aether";


#[tokio::main]
async fn main() {
    println!("[Agent Server] Starting AETHER Agent...");
    

    // Create our API router
    // This defines all the "routes" our server knows
    let app = Router::new().route("/state", post(handle_state));

    let pool = connect_to_db().await;
    println!("[DATABASE] Connected to database.");

    // Define the address to listen on
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("[Agent Server] Listening on http://127.0.0.1:3000");

    // Run the server
    axum::serve(listener, app).await.unwrap();
}

async fn handle_state(Json(payload): Json<HouseholdState>) {

    println!("[Agent Server] Received new household state at {}:", payload.timestamp);
    println!(">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>{:#?}", payload);
    // Insert each appliance's state into the database
}



//connect to  mysql database
async fn connect_to_db()-> Pool<MySql> {
    let pool = MySqlPool::connect(DB_URL).await.unwrap();
    pool
}