use serde::Serialize;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc::Sender; // Import the channel Sender
use tokio::time::{sleep, Duration};
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Clone)]
pub struct ApplianceState {
    pub id: String,
    pub name: String,
    pub watts: f64,
    pub is_on: bool,
}

#[derive(Debug, Serialize)]
pub struct HouseholdState {
    pub timestamp: DateTime<Utc>,
    pub appliances: Vec<ApplianceState>,
}

#[derive(Debug, Clone)]
pub struct ApplianceConfig {
    pub id: String,
    pub name: String,
    pub base_watts: f64,
    // How often this appliance updates its state (in ms)
    pub heartbeat_interval: u64,
}

pub async fn run_appliance(
    config: ApplianceConfig,
    state_sender: Sender<ApplianceState>,
) {
    // Create a thread-safe SmallRng seeded from the current system time (u64).
    // This avoids OsRng/from_rng compatibility issues with older rand versions.
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let seed = now.as_nanos() as u64;
    let mut rng = SmallRng::seed_from_u64(seed);

    // Internal state for this appliance
    let mut is_on = true;
    let mut current_watts = 0.0;

    println!("[Simulator] {} simulation is starting...", config.name);

    loop {
        // --- 2. Simulate Wattage Jitter ---
        if is_on {
            // Calculate jitter: e.g., +/- 5% of base_watts
            let change_percent = rng.random_range(-0.05..0.05);
            let change = config.base_watts * change_percent;
            current_watts = config.base_watts + change;
        } else {
            current_watts = 0.0;
        }

        // --- 3. Create State Payload ---
        let current_state = ApplianceState {
            id: config.id.clone(),
            name: config.name.clone(),
            watts: current_watts,
            is_on,
        };

        // --- 4. Send Heartbeat to `main` Collector ---
        if let Err(e) = state_sender.send(current_state).await {
            println!("[ERROR] Failed to send state for {}: {}. Stopping task.", config.name, e);
            // If the channel is broken, we can stop the task.
            break;
        }

        // Wait for the next heartbeat
        sleep(Duration::from_millis(config.heartbeat_interval)).await;
    }
}
