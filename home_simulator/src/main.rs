use chrono::Utc;
use home_utils::{ApplianceConfig, ApplianceState, HouseholdState, run_appliance};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
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
    rx: mpsc::Receiver<ApplianceState>,
    agent_url: String,
) {
    let client = Client::new();

    // Shared, concurrent map holding the latest state for each appliance
    let state_map: Arc<Mutex<HashMap<String, ApplianceState>>> = Arc::new(Mutex::new(HashMap::new()));

    // Receiver task: consumes incoming appliance states and updates the shared map
    let receiver_map = Arc::clone(&state_map);
    let mut receiver = rx;
    let receiver_handle = tokio::spawn(async move {
        while let Some(app_state) = receiver.recv().await {
            let mut map = receiver_map.lock().await;
            map.insert(app_state.id.clone(), app_state);
            println!("[Collector] Received state update. Total appliances tracked: {}", map.len());
        }
        println!("[Collector] Channel closed, receiver exiting.");
    });

    // Dispatcher task: every interval snapshot the map and send a bundled report
    let dispatcher_map = Arc::clone(&state_map);
    let client_clone = client.clone();
    let agent_url_clone = agent_url.clone();
    let dispatcher_handle = tokio::spawn(async move {
        let mut dispatch_timer = interval(Duration::from_secs(5));
        loop {
            dispatch_timer.tick().await;

            // Take a snapshot of current appliance states
            let appliances: Vec<ApplianceState> = {
                let map = dispatcher_map.lock().await;
                if map.is_empty() {
                    Vec::new()
                } else {
                    map.values().cloned().collect()
                }
            };

            if appliances.is_empty() {
                println!("[Dispatcher] No state received yet. Skipping dispatch.");
                continue;
            }

            let bundled_state = HouseholdState {
                timestamp: Utc::now(),
                appliances,
            };

            match client_clone.post(&agent_url_clone).json(&bundled_state).send().await {
                Ok(res) => {
                    if !res.status().is_success() {
                        eprintln!("[Dispatcher] Agent returned an error: {}", res.status());
                    }
                }
                Err(e) => eprintln!("[Dispatcher] Failed to send state to agent: {}", e),
            }
        }
    });

    // Wait until the receiver task ends (e.g., channel closed). Then stop dispatcher.
    if let Err(join_err) = receiver_handle.await {
        eprintln!("Receiver task panicked: {}", join_err);
    }

    // Shutdown dispatcher task gracefully
    dispatcher_handle.abort();
    let _ = dispatcher_handle.await;

    println!("[Simulator] Collector/Dispatcher shut down.");
}
