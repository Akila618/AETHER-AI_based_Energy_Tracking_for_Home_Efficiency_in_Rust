use tokio::time::{sleep, Duration};
use tokio::sync::{mpsc::Receiver, broadcast};
use aether_utils::AgentMsg;
use chrono::Timelike;
use std::collections::{HashMap, HashSet};
use serde_json::json;

// 1 state iteration (5 seconds) in realtime = 30 minutes in simulation
// daily report interval = 86400 / 1800 = 48 iterations


pub async fn initialize_agent_lookup(mut rx: Receiver<AgentMsg>, bcast: broadcast::Sender<String>){
    println!("[AGENT]: Lookup started.......");
    let mut appliance_map: HashMap<String, u32> = HashMap::new();
    let mut heavy_on: HashSet<String> = HashSet::new();
    let mut duration_map: HashMap<String, u32> = HashMap::new();
    let mut prev_watts: HashMap<String, f64> = HashMap::new();
    let mut latest_watts: HashMap<String, f64> = HashMap::new();
    let mut latest_on: HashMap<String, bool> = HashMap::new();
    let mut last_total: f64 = 0.0;

    loop{
        tokio::select! {
            update = rx.recv() => {
                match update {
                    Some(msg) => {
                        println!("[AGENT] Received appliance state for ID: {} at sim_time: {}", msg.state.id, msg.sim_time);
                        println!("Update recieved: {:#?}", &msg);
                        // =========================================================rules ======================================================
                        println!("[RULES]: Rule engine update recieved.");
                        // alert if bedroom light is on during daylight hours
                        if (&msg.state.name == "Bedroom Light") && *(&msg.state.is_on) {
                            if *&msg.sim_time.hour() >= 6 && *&msg.sim_time.hour() <= 18 {
                                let m = format!("'{}' is ON during daylight hours ({}:00). Please turn off lights to save energy.", msg.state.name, msg.sim_time.hour());
                                println!("[AGENT ALERT]: {}", m);
                                let payload = json!({"type":"alert","severity":"warn","message":m,"appliance": msg.state.name, "watts": msg.state.watts, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                                if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send daylight light alert: {}", e); }
                            }

                        }

                        // alert if AC is running during off-peak hours (match variants like "Living Room AC")
                        let name_l = msg.state.name.to_lowercase();
                        if (name_l.contains("air conditioner") || name_l.contains("ac")) && msg.state.is_on {
                            if msg.sim_time.hour() >= 22 || msg.sim_time.hour() < 6 {
                                println!("[AGENT ALERT]: Air Conditioner (or AC) is running during off-peak hours!");
                            }
                        }

                        // alert if microwave is used for more than 60 minutes continuously
                        if (&msg.state.name == "Microwave") && *(&msg.state.is_on) {
                            if appliance_map.contains_key(&msg.state.id) {
                                let count = appliance_map.get_mut(&msg.state.id).unwrap();
                                *count +=1;
                                if *count >= 2 { 
                                    println!("[AGENT ALERT]: Microwave has been used for more than 60 minutes continuously!");
                                }
                            } else {
                                appliance_map.insert(msg.state.id.clone(), 1);
                            }
                        }

                         // alert if over 300W for refrigerator
                        if (&msg.state.name == "Refrigerator") && (*&msg.state.watts > 300.0) {
                            println!("[AGENT ALERT]: Refrigerator power consumption spike above 300W!");
                        }

                        // if threre any application exceeds the wattage toomuch
                        if *&msg.state.watts > 4000.0 {
                            let m = format!("Peak Load Alert! Total household wattage exceeds 4000W ({}: {:.2}W)", msg.state.name, msg.state.watts);
                            println!("[AGENT ALERT]: {}", m);
                            let payload = json!({"type":"alert","severity":"error","message":m, "appliance": msg.state.name, "watts": msg.state.watts, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                            if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send alert: {}", e); }
                        }


                        // device is OFF but still drawing small power during deep night
                        if !msg.state.is_on && msg.state.watts > 5.0 {
                            let h = msg.sim_time.hour();
                            if h >= 2 && h <= 4 {
                                let m = format!("Possible phantom load: '{}' drawing {:.2}W during 02:00-04:00", msg.state.name, msg.state.watts);
                                println!("[AGENT ALERT]: {}", m);
                                let payload = json!({"type":"alert","severity":"warn","message":m, "appliance": msg.state.name, "watts": msg.state.watts, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                                if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send phantom alert: {}", e); }
                            }
                        }

                        // Sudden spike detection: current > 2x previous 
                        if let Some(prev) = prev_watts.get(&msg.state.id) {
                            if msg.state.watts > prev * 2.0 && (msg.state.watts - prev) > 100.0 {
                                let m = format!("Sudden wattage spike for '{}': prev={:.2}W curr={:.2}W", msg.state.name, prev, msg.state.watts);
                                println!("[AGENT ALERT]: {}", m);
                                let payload = json!({"type":"alert","severity":"warn","message":m, "appliance": msg.state.name, "prev_watts": prev, "watts": msg.state.watts, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                                if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send spike alert: {}", e); }
                            }
                        }
                        prev_watts.insert(msg.state.id.clone(), msg.state.watts);

                        //  heavy appliance on for long simulated duration
                        if msg.state.is_on {
                            let counter = duration_map.entry(msg.state.id.clone()).or_insert(0);
                            *counter += 1;
                            // Using the same heuristic as microwave: >=2 counts ~ long continuous use in simulation
                            if msg.state.watts > 1000.0 && *counter >= 2 {
                                let m = format!("'{}' has been running at high power ({:.2}W) for an extended period.", msg.state.name, msg.state.watts);
                                println!("[AGENT ALERT]: {}", m);
                                let payload = json!({"type":"alert","severity":"warn","message":m, "appliance": msg.state.name, "watts": msg.state.watts, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                                if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send continuous-high alert: {}", e); }
                            }
                        } else {
                            duration_map.insert(msg.state.id.clone(), 0);
                        }

                        // track a set of heavy devices that are currently on
                        let heavy_names = ["Air Conditioner", "Washing Machine", "Dryer", "Oven", "Water Heater"];
                        let is_heavy = heavy_names.iter().any(|n| n == &msg.state.name) || msg.state.watts > 800.0;
                        if msg.state.is_on && is_heavy {
                            heavy_on.insert(msg.state.name.clone());
                            if heavy_on.len() > 1 {
                                let appliances: Vec<String> = heavy_on.iter().cloned().collect();
                                let m = format!("Multiple high-load appliances running simultaneously: {:?}. Consider staggering start times.", appliances);
                                println!("[AGENT ALERT]: {}", m);
                                let payload = json!({"type":"alert","severity":"warn","message":m, "appliances": appliances, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                                if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send overlapping alert: {}", e); }
                            }
                        } else if !msg.state.is_on {
                            heavy_on.remove(&msg.state.name);
                        }

                        // scheduling for washing machine / dryer during peak hours (use case-insensitive contains)
                        if msg.state.is_on {
                            let lower = msg.state.name.to_lowercase();
                            if (lower.contains("washing") || lower.contains("washer") || lower.contains("washing machine") || lower.contains("dryer")) {
                                let h = msg.sim_time.hour();
                                if h >= 7 && h < 22 {
                                    let m = format!("'{}' is running during peak hours ({}:00). Consider scheduling to off-peak to save cost.", msg.state.name, h);
                                    println!("[AGENT SUGGESTION]: {}", m);
                                    let payload = json!({"type":"recommendation","severity":"info","message":m, "appliance": msg.state.name, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                                    if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send recommendation: {}", e); }
                                }
                            }
                        }

                        // battery charging suggestion: prefer solar midday (10-16)
                        let name_low = msg.state.name.to_lowercase();
                        if (name_low.contains("charger") || name_low.contains("ev")) {
                            if msg.state.is_on {
                                let h = msg.sim_time.hour();
                                if h >= 10 && h <= 16 {
                                    let m = format!("'{}' charging during solar hours ({}:00). Good time to charge.", msg.state.name, h);
                                    println!("[AGENT INFO]: {}", m);
                                    let payload = json!({"type":"info","severity":"info","message":m, "appliance": msg.state.name, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                                    if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send info: {}", e); }
                                } else {
                                    let m = format!("'{}' is charging outside solar hours ({}:00). Consider shifting to 10:00-16:00 when solar generation is high.", msg.state.name, h);
                                    println!("[AGENT SUGGESTION]: {}", m);
                                    let payload = json!({
                                        "type":"recommendation",
                                        "severity":"info","message":m,
                                        "appliance": msg.state.name,
                                        "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                                    if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send ev suggestion: {}", e); }
                                }
                            }
                        }

                        // update latest watts/on maps and broadcast live total wattage
                        latest_watts.insert(msg.state.id.clone(), msg.state.watts);
                        latest_on.insert(msg.state.id.clone(), msg.state.is_on);
                        let total: f64 = latest_watts.values().sum();

                        // total threshold crossing alert (only when crossing above)
                        if total > 4000.0 && last_total <= 4000.0 {
                            let m = format!("Household Peak Load: total = {:.2} W (threshold 4000W).", total);
                            println!("[AGENT ALERT]: {}", m);
                            let payload = json!({"type":"alert","severity":"error","message":m, "total_watts": total, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                            if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send household peak alert: {}", e); }
                        }

                        // rapid household increase (delta > 1000W)
                        if (total - last_total) > 1000.0 {
                            let m = format!("Rapid increase in household wattage: +{:.2} W (now {:.2}W)", total - last_total, total);
                            println!("[AGENT ALERT]: {}", m);
                            let payload = json!({"type":"alert","severity":"warn","message":m, "delta": (total - last_total), "total_watts": total, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                            if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send rapid increase alert: {}", e); }
                        }

                        // high load detection: count devices OFF but drawing > 5W
                        let phantom_count = latest_watts.iter().filter(|(id, watts)| {
                            if let Some(is_on) = latest_on.get(*id) {
                                !*is_on && **watts > 5.0
                            } else { false }
                        }).count();
                        if phantom_count >= 3 {
                            let m = format!("Detected higher load across {} devices drawing >5W while OFF.", phantom_count);
                            println!("[AGENT ALERT]: {}", m);
                            let payload = json!({"type":"alert","severity":"warn","message":m, "phantom_count": phantom_count, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                            if let Err(e) = bcast.send(payload) { println!("[BROADCAST] failed to send phantom aggregate alert: {}", e); }
                        }

                        // send live_total after checks
                        let total_msg = json!({"type":"live_total","total_watts": total, "sim_time": msg.sim_time.to_rfc3339()}).to_string();
                        if let Err(e) = bcast.send(total_msg) { println!("[BROADCAST] failed to send live_total: {}", e); }

                        // update last_total for next iteration
                        last_total = total;

                    },
                    None => {
                        println!("[AGENT] Agent channel closed, stopping lookup.");
                        break;
                    }
                }
            }
            _ = sleep(Duration::from_secs(1)) => {
                println!("[AGENT]: Running periodic lookup...");
            }
        }
    }
}

// #[allow(dead_code)]
// async fn apply_rules(update: &AgentMsg) {
//     println!("[RULES]: Rule engine update recieved.");
//     // alert if bedroom light is on during daylight hours
//     if (&update.state.name == "Bedroom Light") && *(&update.state.is_on) {
//         if *&update.sim_time.hour() >= 6 && *&update.sim_time.hour() <= 18 {
//             println!("[AGENT ALERT]: Bedroom light is on during daylight hours!");
//         }

//     }

//     // alert if AC is running during off-peak hours
//     if (&update.state.name == "Air Conditioner") && *(&update.state.is_on) {
//         if *&update.sim_time.hour() >= 22 || *&update.sim_time.hour() < 6 {
//             println!("[AGENT ALERT]: Air Conditioner is running during off-peak hours!");
//         }
//     }

//     // alert if microwave is used for more than 60 minutes continuously
//     if (&update.state.name == "Microwave") && *(&update.state.is_on) {
        
//     }

// }