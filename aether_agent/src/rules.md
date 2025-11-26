# AETHER Agent Rules

This document lists the active rules implemented in `aether_agent/src/rules.rs`. For each rule: trigger conditions, severity, what the agent does (log vs broadcast), and example payload fields are described.

---

## Global state used by rules
- `appliance_map: HashMap<String,u32>` — per-appliance counters (used for continuous-use heuristics, e.g., microwave).
- `heavy_on: HashSet<String>` — currently-on heavy appliances set (used to detect overlapping heavy appliances).
- `duration_map: HashMap<String,u32>` — per-appliance continuous-on counters (simulation steps).
- `prev_watts: HashMap<String,f64>` — previous reading per-appliance (used for spike detection).
- `latest_watts: HashMap<String,f64>` — most recent watts per appliance (used to compute household total & aggregated phantom load)
- `latest_on: HashMap<String,bool>` — most recent on/off state per appliance
- `last_total: f64` — last computed household total watts (used to detect threshold crossings and rapid changes)

---

## Rules (implemented)

1. Bedroom Light — Daylight ON
- Trigger: `msg.state.name == "Bedroom Light" && msg.state.is_on && hour in [6..=18]`
- Severity: `warn`
- Action: Broadcasts an `alert` payload recommending turning the light off.
- Payload example: `{ type: "alert", severity: "warn", message, appliance, watts, sim_time }

2. Air Conditioner — Off-peak running (console-only)
- Trigger: `msg.state.name == "Air Conditioner" && msg.state.is_on && (hour >= 22 || hour < 6)`
- Severity: N/A (currently printed to agent logs only)
- Action: `println!` log message (no broadcast in the current implementation).
- Note: can be extended to broadcast a recommendation/alert.

3. Microwave — Continuous usage > ~60 minutes (console-only)
- Trigger: `msg.state.name == "Microwave" && msg.state.is_on` and appliance counter >= 2 (2 simulation steps ≈ 60 minutes)
- Severity: N/A (currently printed to agent logs only)
- Action: `println!` log warning.
- Note: the code increments `appliance_map` per state update; can be changed to broadcast if desired.

4. Refrigerator — Per-device high-watt spike (console-only)
- Trigger: `msg.state.name == "Refrigerator" && msg.state.watts > 300.0`
- Severity: N/A (currently printed to agent logs only)
- Action: `println!` log message.
- Note: sudden-spike rule (below) also detects major spikes and does broadcast alerts for any device.

5. Per-appliance Peak/Wattage > 4000W
- Trigger: `msg.state.watts > 4000.0` (per single appliance reading)
- Severity: `error`
- Action: Broadcasts an `alert` payload indicating a peak load from that appliance.
- Payload example: `{ type: "alert", severity: "error", message, appliance, watts, sim_time }
- Note: This is a per-appliance safety check; there is also a household-total peak rule (see rule 12).

6. Phantom load (single-device, deep-night)
- Trigger: `!msg.state.is_on && msg.state.watts > 5.0 && hour in [2..=4]`
- Severity: `warn`
- Action: Broadcasts an `alert` about a possible phantom/standby load for that device.
- Payload example: `{ type: "alert", severity: "warn", message, appliance, watts, sim_time }

7. Sudden per-device spike detection
- Trigger: If a previous reading exists and `curr > prev * 2.0 && (curr - prev) > 100.0`
- Severity: `warn`
- Action: Broadcasts an `alert` describing the sudden spike and includes `prev_watts` and `watts`.
- Payload example: `{ type: "alert", severity: "warn", message, appliance, prev_watts, watts, sim_time }

8. Continuous high-consumption detection
- Trigger: `msg.state.is_on && msg.state.watts > 1000.0` and `duration_map[appliance] >= 2` (extended period)
- Severity: `warn`
- Action: Broadcasts an `alert` that a heavy appliance is running at high power for an extended period.
- Payload example: `{ type: "alert", severity: "warn", message, appliance, watts, sim_time }

9. Overlapping heavy appliances
- Trigger: Appliance is in `heavy_names = ["Air Conditioner","Washing Machine","Dryer","Oven","Water Heater"]` OR `msg.state.watts > 800.0`, and multiple heavy appliances are currently on (tracked in `heavy_on` set)
- Severity: `warn`
- Action: Broadcasts an `alert` listing the heavy appliances running simultaneously and suggests staggering start times.
- Payload example: `{ type: "alert", severity: "warn", message, appliances, sim_time }

10. Washing Machine / Dryer — Scheduling suggestion
- Trigger: `(name == "Washing Machine" || name == "Dryer") && is_on && hour in [7..22)` (i.e., running during peak daytime hours)
- Severity: `info` (emitted as `recommendation`)
- Action: Broadcasts a `recommendation` advising the user to schedule to off-peak hours.
- Payload example: `{ type: "recommendation", severity: "info", message, appliance, sim_time }

11. EV Charger / Battery charging suggestions
- Trigger: `name contains "Charger" || name contains "EV"` and device is on
- Severity: `info` (either `info` or `recommendation` depending on time)
- Action:
  - If hour in [10..=16] → broadcast `info` noting charging during solar hours (good)
  - Else → broadcast `recommendation` to shift charging to solar hours
- Payload examples:
  - Info: `{ type: "info", severity: "info", message, appliance, sim_time }
  - Recommendation: `{ type: "recommendation", severity: "info", message, appliance, sim_time }

12. Household total (computed from `latest_watts`) — Live total
- Trigger: updated every state update after `latest_watts` is updated
- Severity: n/a (informational)
- Action: Broadcasts a `live_total` message containing the current household `total_watts` and `sim_time`.
- Payload example: `{ type: "live_total", total_watts, sim_time }

13. Household total threshold crossing
- Trigger: `total > 4000.0 && last_total <= 4000.0` (detect crossing upward)
- Severity: `error`
- Action: Broadcasts an `alert` that household total crossed the 4000W threshold.
- Payload example: `{ type: "alert", severity: "error", message, total_watts, sim_time }

14. Rapid household increase
- Trigger: `total - last_total > 1000.0` (large jump since last measurement)
- Severity: `warn`
- Action: Broadcasts an `alert` describing the rapid increase; includes `delta` and `total_watts`.
- Payload example: `{ type: "alert", severity: "warn", message, delta, total_watts, sim_time }

15. Aggregated phantom-load detection
- Trigger: Count of devices for which `(latest_on[id] == false) && (latest_watts[id] > 5.0)` >= 3
- Severity: `warn`
- Action: Broadcasts an `alert` indicating multiple devices appear to be drawing >5W while OFF.
- Payload example: `{ type: "alert", severity: "warn", message, phantom_count, sim_time }

---

## Notes, behavior & suggestions
- Some early checks (AC off-peak, microwave continuous, refrigerator >300W) are currently `println!` logs and do not broadcast. There are other rules that broadcast similar conditions (e.g., sudden spike detection covers general spikes for any device and does broadcast).
- `live_total` is always sent after the rule checks so the frontend has a frequent, accurate household total to render.
- To avoid repeated identical alerts spamming the UI, consider adding a cooldown per-alert-type (e.g., suppress identical `type+appliance` alerts for N minutes) or deduplication with timestamps.
- Severity mapping in frontend currently uses `warn` and `error` to style alerts; recommendations and info messages are rendered in the recommendations panel. You can tune the mapping or add more granular severities.

---

If you want, I can:
- Add cooldown/de-duplication logic to rules to reduce repeated alerts.
- Convert log-only checks (AC, microwave, refrigerator) to broadcast messages.
- Add unit tests for rule triggers using a small set of synthetic `AgentMsg` inputs.

Created by automation on behalf of the developer.
