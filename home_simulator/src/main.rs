use chrono::Utc;
use home_utils::{ApplianceConfig, ApplianceState, HouseholdState, run_appliance};
use reqwest::Client;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

#[tokio::main]
async fn main() {
    println!("Starting AETHER House Simulator...");

    let agent_url = "http://127.0.0.1:3000/state";

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
    let (tx, rx) = mpsc::channel(100); // Channel with a buffer of 100

    // --- Spawn an async task for each appliance ---
    for app_config in appliances_to_simulate {
        let tx_clone = tx.clone(); // Clone the sender for this task
        tokio::spawn(async move {
            run_appliance(app_config, tx_clone).await;
        });
    }

    // --- Start the Collector & Dispatcher Loop ---
    println!("[Simulator] All appliance tasks spawned. Starting collector loop...");
    run_collector_dispatcher(rx, agent_url.to_string()).await;
}

async fn run_collector_dispatcher(
    mut state_receiver: mpsc::Receiver<ApplianceState>,
    agent_url: String,
) {
    let client = Client::new();

    // This HashMap stores the *most recent* state of every appliance
    let mut household_state_map: HashMap<String, ApplianceState> = HashMap::new();

    // Send a bundled report to the agent every 5 seconds
    let mut dispatch_timer = interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            Some(app_state) = state_receiver.recv() => {

                household_state_map.insert(app_state.id.clone(), app_state);
                println!("[Collector] Received state update. Total appliances tracked: {}", household_state_map.len());
                println!("[Latest State] {:#?}", &household_state_map);
            }

            _ = dispatch_timer.tick() => {
                if household_state_map.is_empty() {
                    println!("[Dispatcher] No state received yet. Skipping dispatch.");
                    continue;
                }

                println!("[Dispatcher] 5s timer ticked. Sending bundled state to agent...");

                // Bundle all states from the map into a Vec
                let appliances: Vec<ApplianceState> =
                    household_state_map.values().cloned().collect();

                let bundled_state = HouseholdState {
                    timestamp: Utc::now(),
                    appliances,
                };

                // --- Send the single, bundled report ---
                match client.post(&agent_url).json(&bundled_state).send().await {
                    Ok(res) => {
                        if !res.status().is_success() {
                            println!("[Dispatcher] Agent returned an error: {}", res.status());
                        }
                    },
                    Err(e) => {
                        println!("[Dispatcher] Failed to send state to agent: {}", e);
                    }
                }
            }
        }
    }
}
