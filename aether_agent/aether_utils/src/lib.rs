use std::clone;

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

pub const AGENT_VERSION: &str = "AETHER Agent v0.1.0";

//weekly report interval in seconds
// 1 state iteration (5 seconds) in realtime = 30 minutes in simulation = 1800 seconds
// 1 week = 7 days = 60*60*24*7 = 604800 seconds
// weekly report interval = 604800 / 1800 = 336 iterations
// daily report interval = 86400 / 1800 = 48 iterations

pub const STATE_PASS_INTERVAL_SECS: u64 = 5;
pub const SIMULATED_TIME_MINS: u64 = 30;
pub const SIMULATED_TIME_SECS: u64 = SIMULATED_TIME_MINS * 60;
pub const SIMULATED_TIME_HOURS: u64 = SIMULATED_TIME_MINS / 60;

pub const DAILY_REPORT_ITERATIONS: u64 = 86400 / SIMULATED_TIME_SECS;
pub const WEEKLY_REPORT_ITERATIONS: u64 = DAILY_REPORT_ITERATIONS*7;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApplianceState {
    pub id: String,
    pub name: String,
    pub watts: f64,
    pub is_on: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HouseholdState {
    pub timestamp: DateTime<Utc>,
    pub appliances: Vec<ApplianceState>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentMsg {
    pub state: ApplianceState,
    pub real_time: DateTime<Utc>,
    pub sim_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplianceConfig {
    pub id: String,
    pub name: String,
    pub base_watts: f64,
    pub heartbeat_interval: u64,
}