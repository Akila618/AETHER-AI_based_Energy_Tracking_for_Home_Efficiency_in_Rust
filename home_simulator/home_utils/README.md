# AETHER: AI-based Energy Tracking for Home Efficiency (in) Rust

AETHER is a high-performance smart energy monitoring system built entirely in Rust. This project uses a client-server architecture to simulate a household's energy consumption and provide intelligent, rule-based recommendations for improving efficiency.

This repository contains the **`house_simulator` (the Client)**. Its job is to:
1.  Simulate multiple home appliances (AC, lights, fridge) in parallel as independent `async` tasks.
2.  Have each appliance manage its own state (`is_on`) and simulate realistic, fluctuating power draw ("jitter").
3.  Send "heartbeat" updates from each appliance to a central collector using Tokio's MPSC channels.
4.  Bundle all appliance states into a single `HouseholdState` JSON payload.
5.  Send this bundled report to the `smart_agent` server every 5 seconds.

The companion `smart_agent` (Server) project is responsible for receiving this data and applying AI rules.

---

## Project Architecture (`house_simulator`)

This program uses a "Collector" pattern to efficiently manage multiple concurrent simulations.

1.  **Appliance Tasks:** Independent `async` functions are spawned for each appliance. Each task runs its own logic (e.g., turning on/off, calculating power jitter).
2.  **MPSC Channel:** Each task sends its `ApplianceState` heartbeat to an in-memory "Multi-Producer, Single-Consumer" (MPSC) channel.
3.  **Collector (main):** The `main` function acts as the *single consumer*. It listens for heartbeats and updates a `HashMap` with the latest state for each appliance.
4.  **Dispatcher (main):** A 5-second `tokio::Interval` in `main` triggers a dispatch. It bundles all states from the `HashMap` into one `HouseholdState` and sends it to the `smart_agent` via an HTTP POST request.

```ascii
+-------------------------------------------------------------------------+
|                       PROGRAM 1: house_simulator                        |
+=========================================================================+
|                                                                         |
|  +-----------------------+      +-----------------------+               |
|  | async fn run_appliance|      | async fn run_appliance| (x5 Tasks)    |
|  | (AC Logic)            |      | (Lights Logic)        |               |
|  | [is_on, watts_jitter] |      | [is_on, watts_jitter] |               |
|  +----------|------------+      +----------|------------+               |
|             |                        |                                  |
|   (1) Send Heartbeat   (1) Send HeartG                                  |
|   (ApplianceState)     (ApplianceState)                                 |
|             |                        |                                  |
|             v                        v                                  |
|  +----------+------------------------+-----------------------+          |
|  |        Tokio MPSC Channel (In-Memory Message Bus)         |          |
|  +-----------------------------------|-----------------------+          |
|                                    | (2) Receive States                 |
|                                    v                                    |
|  +-----------------------------------+-----------------------+          |
|  |                async fn main() (Collector)                |          |
|  |                                                         |            |
|  |  [ HashMap<id, ApplianceState> ] (Stores latest state)    |          |
|  |                 |                                         |          |
|  |  [ tokio::Interval (5s) ] (Triggers dispatch)             |          |
|  |                 |                                         |          |
|  |                 v                                         |          |
|  |  [ 3. Send to Agent (reqwest) ]                           |          |
|  |      (Bundles all states into one HouseholdState)         |          |
|  +-----------------|-----------------------------------------+          |
|                    |                                                    |
|                    v (HTTP POST to [http://127.0.0.1:3000/state])       |
|                                                                         |
+--------------------AETHER SERVER ---------------------------------------+
