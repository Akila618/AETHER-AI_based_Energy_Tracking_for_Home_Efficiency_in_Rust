use chrono::Utc;
use home_utils::{ApplianceConfig, ApplianceState, HouseholdState, run_appliance, STATE_PASS_INTERVAL_SECS};
use reqwest::Client;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tokio::time::sleep;

// ============================= async (tokio) main function ===========================================
#[tokio::main]
async fn main() {
    println!("Starting AETHER House Simulator...");

    // =================== define our list of appliances ====================================
    let appliances_to_simulate = vec![
        ApplianceConfig {
            id: "ac_living".to_string(),
            name: "Living Room AC".to_string(),
            base_watts: 1500.0,
            heartbeat_interval: 2000,
            is_on: true,
        },
        ApplianceConfig {
            id: "light_kitchen".to_string(),
            name: "Kitchen Light".to_string(),
            base_watts: 100.0,
            heartbeat_interval: 1000,
            is_on: false,
        },
        ApplianceConfig {
            id: "tv_living".to_string(),
            name: "Living Room TV".to_string(),
            base_watts: 200.0,
            heartbeat_interval: 1500,
            is_on: true,
        },
        ApplianceConfig {
            id: "fridge".to_string(),
            name: "Refrigerator".to_string(),
            base_watts: 200.0,
            heartbeat_interval: 3000,
            is_on: true,
        },
        ApplianceConfig {
            id: "microwave".to_string(),
            name: "Microwave".to_string(),
            base_watts: 1100.0,
            heartbeat_interval: 5000,
            is_on: false,
        },
        ApplianceConfig {
            id: "washer".to_string(),
            name: "Washing Machine".to_string(),
            base_watts: 500.0,
            heartbeat_interval: 4000,
            is_on: false,
        },
        ApplianceConfig {
            id: "dryer".to_string(),
            name: "Clothes Dryer".to_string(),
            base_watts: 3000.0,
            heartbeat_interval: 6000,
            is_on: false,
        },
        ApplianceConfig {
            id: "oven".to_string(),
            name: "Oven".to_string(),
            base_watts: 2400.0,
            heartbeat_interval: 7000,
            is_on: false,
        },
        ApplianceConfig {
            id: "light_bedroom".to_string(),
            name: "Bedroom Light".to_string(),
            base_watts: 75.0,
            heartbeat_interval: 1000,
            is_on: true,
        },
        ApplianceConfig {
            id: "computer".to_string(),
            name: "Desktop Computer".to_string(),
            base_watts: 250.0,
            heartbeat_interval: 1500,
            is_on: true,
        },
    ];

    // appliances will send data to 1 consumer (main loop)
    let (tx, rx) = mpsc::channel(150);


    // spawn an async task for each appliance ---
    for app_config in appliances_to_simulate {
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            run_appliance(app_config, tx_clone).await;
        });
    }

    // drop the original sender so that when all cloned senders (in tasks) are dropped,
    drop(tx);

    // start the state collection & sending loop 
    println!("[Simulator] All appliance tasks spawned. Starting dispatcher loop...");
    let agent_url = "http://127.0.0.1:3000/state".to_string();
    run_collection_sender(rx, agent_url).await;
    
}

async fn run_collection_sender(
    mut state_receiver: mpsc::Receiver<ApplianceState>,
    agent_url: String,
) {
    let client = connect_to_server().await;

    // latest appliance state tracked by appliance id
    let mut household_state_map: HashMap<String, ApplianceState> = HashMap::new();

    // send a complete report to the agent every 5 seconds
    let mut dispatch_timer = interval(Duration::from_secs(STATE_PASS_INTERVAL_SECS as u64));
    loop {
        tokio::select! {
            Some(app_state) = state_receiver.recv() => {
                // update the latest state for this appliance
                household_state_map.insert(app_state.id.clone(), app_state);
                println!("[Collector] Received state update. Appliances tracked: {}", household_state_map.len());
            }

            _ = dispatch_timer.tick() => {
                if household_state_map.is_empty() {
                    println!("[Sender] No state received yet....... waiting....");
                    continue;
                }

                println!("[Sender] 5s passed. Sending home state to agent...");

                // Bundle all states from the map into a Vec
                let appliances: Vec<ApplianceState> = household_state_map.values().cloned().collect();

                let bundled_state = HouseholdState {
                    timestamp: Utc::now(),
                    appliances,
                };

                // send the single, bundled report
                match client.post(&agent_url).json(&bundled_state).send().await {
                    Ok(res) => {
                        if !res.status().is_success() {
                            println!("[Sender] Agent returned an error: {}", res.status());
                        } else {
                            println!("[Sender] Successfully sent house state ({} appliances)", bundled_state.appliances.len());
                        }
                    },
                    Err(e) => {
                        println!("[Sender] Failed to send state to agent: {}", e);
                    }
                }
            }

            else => {
                println!("[Sender] No more senders; exiting dispatcher loop.");
                break;
            }
        }
    }
}

// connect to server and send callback
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
