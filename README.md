# AETHER: AI-based Energy Tracking for Home Efficiency (in) Rust/React

An intelligent system designed to monitor, predict, and optimize household electricity usage. Developed under the EEX7340/EEX6340 - AI Techniques and Agent Technology module, this project implements a Rational Intelligent Agent using a highly efficient, concurrent architecture built primarily in Rust.

The Agent's core function is to enforce energy-efficiency rules and provide accurate monthly consumption forecasts.

## 1. System Architecture and Data Flow

The system operates as a decoupled architecture where three independent services work together using asynchronous communication protocols.

System Overview

- Ingestion: The Home Simulator (client) sends state snapshots via HTTP POST to the Agent's `/state` endpoint.
- Persistence: The Aether Agent (server) receives the data and persists it to a MySQL database.
- Real-Time Push: The Agent broadcasts alerts and live wattage totals via WebSocket to the Web UI.

### Technical Stack

Component | Role | Primary Technology | Concurrency Model
---|---:|---|---
Agent (`aether_agent`) | Rule engine, prediction, persistence | Rust, Axum, Tokio, SQLx | Async (Tokio)
Simulator (`home_simulator`) | Telemetry generator / client | Rust | Async task spawning
Prediction Module | Forecasting | Linfa, ndarray (Rust) | Linear regression training
Frontend (`web-ui`) | User interface | React, Vite | WebSocket client

## 2. Core Intelligence and Logic

The Agent's intelligence layer provides two primary capabilities, satisfying the need for both immediate reactivity and long-term prediction.

### 2.1 Rule Engine (Reactive Intelligence)

- Peak Load Alert: Instantly alerts the user if the total household wattage exceeds a safety threshold (for example, 4000 W).
- Night-time/Daylight Waste: Checks simulated time to alert users about lights left on during daylight hours or AC running during unfavorable off-peak times.
- Temporal Checks: Tracks continuous usage (for example, microwave running for >60 simulated minutes) and flags potential equipment issues.

### 2.2 Predictive Module (Forecasting)

- Model: Linear Regression is used for forecasting total monthly consumption.
- Feature Engineering: The model is trained on cyclical features extracted from the database, primarily Day of Week and Month of Year, to capture seasonal usage patterns.
- Output: Predicts total kWh consumption and estimated cost (LKR) for the target month.

## 3. Setup and Execution

The project requires three components to be launched independently in separate terminal windows.

### Prerequisites

- Rust toolchain (stable) and Cargo
- Node.js and npm (for the React frontend)
- MySQL Server (local instance recommended)

### Execution Steps

1. Set Database URL: Ensure your MySQL connection string is correctly configured for the Agent (edit `aether_agent/src/main.rs` or replace the `DB_URL` constant with your credentials or use an environment configuration).

2. Start the Agent (Server): This initiates the server, database connection pool, model training, and the periodic rule engine task.

PowerShell
```
# Terminal 1 (AETHER AGENT)
Set-Location -LiteralPath 'aether_agent'
cargo run
```

3. Start the Simulator (Client): This begins pushing simulated data to the Agent's `/state` endpoint every few seconds.

PowerShell
```
# Terminal 2 (HOME SIMULATOR)
Set-Location -LiteralPath 'home_simulator'
cargo run
```

4. Start the Web UI (Frontend): This launches the React dashboard, which automatically connects to the Agent via WebSocket for live updates.

PowerShell
```
# Terminal 3 (WEB UI)
Set-Location -LiteralPath 'web-ui'
npm install
npm run dev
```

Once running, the Agent's console will log database insertions and rule evaluations, and the Web UI will display real-time wattage updates, alerts, predictions, and recommendations.

## 4. API Endpoints (Selected)

- `POST /state` — accepts a `HouseholdState` payload from the simulator (appliance snapshots). 
- `GET /ws` — WebSocket endpoint; clients receive broadcasted JSON messages (`type`: `state`, `alert`, `recommendation`, `info`, `live_total`).
- `POST /api/chat` — natural-language chat endpoint; accepts JSON `{ "query": "..." }` and replies with `{ "reply": "..." }`.
- `GET /api/predictions?year=YYYY&month=M` — returns monthly forecast JSON (monthly_kwh, monthly_cost, daily_watts, ...).

## 5. Development Notes & Tips

- The rule engine uses heuristic matching on appliance names; simulator device naming should be consistent with rules (or rules can be made case-insensitive / substring-based).
- If recommendations do not appear in the UI, ensure the Agent is broadcasting messages of type `recommendation` or `info` and the frontend WebSocket is connected.
- Use the `aether_utils` crate types to ensure simulator and agent payloads remain compatible.
