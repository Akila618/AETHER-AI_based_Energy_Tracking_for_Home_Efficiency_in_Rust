use chrono::Utc;
use home_utils::{ApplianceConfig, ApplianceState, HouseholdState, run_appliance};
use reqwest::Client;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tokio::time::sleep;

//cient for sending HTTP requests to aether agent



#[tokio::main]
async fn main() {
    println!("Starting AETHER House Simulator...");

    // --- Define our list of appliances ---
    let appliances_to_simulate = vec![
        ApplianceConfig {
            id: "ac_living".to_string(),
            name: "Living Room AC".to_string(),
            base_watts: 1500.0,
            heartbeat_interval: 2000,
        },
        ApplianceConfig {
            id: "light_kitchen".to_string(),
            name: "Kitchen Light".to_string(),
            base_watts: 100.0,
            heartbeat_interval: 1000,
        },
        ApplianceConfig {
            id: "tv_living".to_string(),
            name: "Living Room TV".to_string(),
            base_watts: 200.0,
            heartbeat_interval: 1500,
        },
        ApplianceConfig {
            id: "fridge".to_string(),
            name: "Refrigerator".to_string(),
            base_watts: 200.0,
            heartbeat_interval: 3000,
        },
        ApplianceConfig {
            id: "microwave".to_string(),
            name: "Microwave".to_string(),
            base_watts: 1100.0,
            heartbeat_interval: 5000,
        },
    ];

    // n producers (appliances) will send data to 1 consumer (main loop).
    let (tx, rx) = mpsc::channel(150);


    // --- Spawn an async task for each appliance ---
    for app_config in appliances_to_simulate {
        let tx_clone = tx.clone(); // Clone the sender for this task
        tokio::spawn(async move {
            run_appliance(app_config, tx_clone).await;
        });
    }

    // --- Start the state collection loop ---
    println!("[Simulator] All appliance tasks spawned. Starting collector loop...");
    run_state_collector(rx).await;
    
}

async fn run_state_collector(mut state_receiver: mpsc::Receiver<ApplianceState>) {
    let client = connect_to_server().await;

    while let Some(state) = state_receiver.recv().await {
        //println!("[Collector] Received update: {:?}:{:?}",state.name, state.watts);
        // Send the current household state to the AETHER agent
        let household_state = HouseholdState {
            timestamp: Utc::now(),
            appliances: vec![state],
        };
        send_household_state(client.clone(), household_state).await;
        
    }
}

//connect to server and send callback
async fn connect_to_server() -> Client {
    let client = Client::new();
    loop {
        if client.get("http://127.0.0.1:3000/state").send().await.is_ok() {
            println!("[Simulator] Connected to AETHER Agent server.");
            return client;
            
        } else {
            println!("[Simulator] Failed to connect to AETHER Agent server. Retrying in 2 seconds...");
            sleep(Duration::from_secs(2)).await;
        }
    }
}

//send household state to server
async fn send_household_state(client: Client, state: HouseholdState) {
    let res = client.post("http://127.0.0.1:3000/state").json(&state).send().await; 
    if res.is_ok() {
        println!("[Simulator] Sent household state to AETHER Agent.");
    } else {
        println!("[Simulator] Failed to send household state to AETHER Agent.");
    }
}