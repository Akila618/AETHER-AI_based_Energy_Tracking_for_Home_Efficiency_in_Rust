# AETHER Agent (aether_agent)

AETHER Agent is the central server that ingests simulated household telemetry, persists it to a MySQL database, evaluates rule-based heuristics for alerts and recommendations, and provides a REST prediction API and WebSocket stream for real-time UI updates.

This README mirrors the structure and tone of the `house_simulator` README so developers can quickly understand how to run, extend, and test the Agent.

---

## What the Agent does

1. Receives `HouseholdState` JSON POSTs from the simulator at `POST /state`.
2. Persists individual `ApplianceState` rows into a MySQL `home_state` table (columns: `id`, `name`, `watts`, `is_on`, `timestamp`, `sim_timestamp`).
3. Forwards incoming messages into an in-process `mpsc` channel consumed by the rule engine.
4. Runs an asynchronous rules engine that emits structured JSON messages (types: `state`, `alert`, `recommendation`, `info`, `live_total`) into a `broadcast` channel.
5. Exposes a WebSocket endpoint at `GET /ws` that subscribes clients to the broadcast channel, delivering live updates to the `web-ui`.
6. Provides a REST endpoint `GET /api/predictions?year=YYYY&month=MM` which trains (on-demand) a simple linear regression model and returns JSON `{ year, month, monthly_kwh, monthly_cost, daily_watts }`.

---

## Project layout

- `src/main.rs` - HTTP server (Axum) and routing (`/state`, `/ws`, `/api/predictions`).
- `src/rules.rs` - Rules engine consuming `AgentMsg` messages and publishing alerts/recommendations.
- `src/models.rs` - Prediction training and inference helpers (Linfa + ndarray).
- `Cargo.toml` - Rust crate manifest with dependencies (Axum, Tokio, SQLx, Linfa, etc.).

---

## Endpoints and message formats

- POST /state
  - Receives `HouseholdState` JSON. Example payload format matches the simulator `HouseholdState` type (timestamp + array of `ApplianceState`). The Agent persists each appliance entry.

- GET /ws
  - WebSocket upgrade. Clients receive JSON messages with a `type` field. Example message types:
    - `state`: raw appliance snapshot
    - `alert`: urgent issue (red)
    - `recommendation`: suggestion (green)
    - `info`: informational messages
    - `live_total`: aggregated household wattage

- GET /api/predictions?year=YYYY&month=MM
  - Returns a JSON object:
    {
      "year": 2025,
      "month": 11,
      "monthly_kwh": 123.45,
      "monthly_cost": 987.65,
      "daily_watts": [ 345.0, 300.2, ... ]
    }
  - Note: the prediction logic trains a simple linear model on daily aggregates. In development the handler sets liberal CORS headers for convenience; in production tighten origins and enable HTTPS.

---

## How to run (development)

1. Ensure a local MySQL instance is running and available. Create a database and note the connection URL.
2. From the `aether_agent` folder, set `DB_URL` environment variable and run:

```powershell
# from repository root
Set-Item -Path Env:DB_URL -Value "mysql://user:password@127.0.0.1:3306/aether_db"
cd aether_agent
cargo run
```

3. The agent listens by default on `127.0.0.1:3000`.
4. Start `home_simulator` to POST telemetry and the `web-ui` to observe live updates.

---

## Development notes

- The rules engine runs as an async task spawned at startup. It expects to receive `AgentMsg` values via an `mpsc::Receiver` and to publish JSON strings with a `broadcast::Sender`.
- The prediction model uses `linfa-linear` and trains on daily aggregates built from `home_state`. Training happens on-demand and can be slow for large datasets — consider caching the trained model or moving training to a background worker.
- For quick local testing, liberal CORS is applied on the prediction handler. Replace with a proper CORS middleware and restrict origins in production.

---

## Debugging & testing tips

- To reproduce rule scenarios, send crafted `HouseholdState` payloads to `POST /state` (e.g., multiple high-watt appliances to trigger `live_total` alerts).
- Use `cargo check` to validate the Rust project. Fix compiler warnings and unused imports to keep the codebase clean.
- Unit tests: consider adding tests around `rules.rs` by constructing `AgentMsg` inputs and asserting expected outputs.

---

## Useful files

- `src/main.rs` - startup and routing
- `src/rules.rs` - rules and broadcast logic
- `src/models.rs` - prediction helpers

---

If you'd like, I can also:
- Add a short `examples/` folder with `curl` examples for `POST /state` and `GET /api/predictions`.
- Add PowerShell run scripts mirroring `run_sim.ps1` to start the Agent with a default `.env`.

---

Generated to match the `house_simulator` README style. Let me know if you want additional diagrams, example payloads, or a git commit & branch created.

```ascii
+-------------------------------------------------------------------------+
|                       PROGRAM: aether_agent                              |
+=========================================================================+
|                                                                         |
|  +----------------------+      +-----------------------+      +--------+|
|  |  HTTP Server (Axum)  | ---> |  Database (MySQL)     | <--- | Model  ||
|  |  (/state, /api/...)  |      |  table: home_state    |      | Module ||
|  +----------+-----------+      +-----------------------+      +---+----+|
|             |                                                   ^    |
|             | POST /state                                      /     |
|             v                                              train /     |
|  +----------------------+      mpsc::Sender      +----------------------+|
|  |  Message Bus         | --------------------> |  Rules Engine (task)  ||
|  |  (mpsc + broadcast)  |                       |  evaluates rules,     ||
|  +----------+-----------+                       |  emits alerts/recs    ||
|             |                                   +----------+-----------+||
|             | broadcast JSON                              |           ||
|             v                                             v           ||
|  +----------------------+                                  +--------+  ||
|  | WebSocket Handler    | <-----------------------------+  (broadcast::) ||
|  |  (/ws) subscribes to |                                    sends JSON   ||
|  |  broadcast channel   |                                    to clients   ||
|  +----------------------+                                                   |
|                                                                         |
+--------------------AGENT COMPONENT INTERACTIONS--------------------------+

``` 