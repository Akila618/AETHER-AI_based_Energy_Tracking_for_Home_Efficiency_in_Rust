use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplianceConfig {
    pub id: String,
    pub name: String,
    pub base_watts: f64,
    pub heartbeat_interval: u64,
}