use serde::Serialize;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, Duration};
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use std::time::{SystemTime, UNIX_EPOCH};

pub const STATE_PASS_INTERVAL_SECS: i64 = 5;

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
    pub heartbeat_interval: u64,
    pub is_on: bool,
}

pub async fn run_appliance( config: ApplianceConfig, state_sender: Sender<ApplianceState>) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let seed = now.as_nanos() as u64;
    let mut rng = SmallRng::seed_from_u64(seed);

    let mut current_watts = 0.0;

    println!("[Simulator] {} simulation is starting...", config.name);

    loop {

        // change the is on (true/false) state randomly
        let is_on_rand: bool = if rng.random_range(0..150) > 75 { true } else { false };

        // simulate Wattage change based on the generated on/off state
        if is_on_rand {
            let change_percent = rng.random_range(-0.05..0.05);
            let change = config.base_watts * change_percent;
            current_watts = config.base_watts + change;
        } else {
            current_watts = 0.0;
        }

        // create the changes application state
        let current_state = ApplianceState {
            id: config.id.clone(),
            name: config.name.clone(),
            watts: current_watts,
            is_on: is_on_rand,
        };

        // send heartbeat to `main` thread collector
        if let Err(e) = state_sender.send(current_state).await {
            println!("[ERROR] Failed to send state for {}: {}. Stopping task.", config.name, e);
            break;
        }

        // wait for the next heartbeat
        sleep(Duration::from_millis(config.heartbeat_interval)).await;
    }
}
