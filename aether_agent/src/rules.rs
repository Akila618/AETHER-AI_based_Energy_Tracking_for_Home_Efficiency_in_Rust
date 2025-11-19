use tokio::time::{sleep, Duration};
use tokio::sync::mpsc::Receiver;
use aether_utils::AgentMsg;
use chrono::Timelike;
use std::collections::{HashMap, HashSet};
use crate::models;
/*

@@ Rules for AETHER Energy Agent

- Print a "Peak Load Alert" if the total household wattage exceeds 4000W.
- Show a alert if bulbs are on during daylight hours (6 AM to 6 PM).
- Show AC notification when it's running during off-peak hours (10 PM to 6 AM).
- If the microwave is used for more than 60 minutes continuously, print a warning.
- If the washing machine and dryer run simultaneously, print a notification suggesting to stagger their usage.
- If the refrigerator's power consumption spikes above 300W, print an alert.

- Analyze the past simulated week to identify and report the most energy-intensive appliance (e.g., "The AC was the most expensive").
- Calculate the total estimated energy cost (in LKR) for the past simulated 30 days and report the potential savings.

- Analyze the lowest wattage used during off-peak simulated hours (e.g., 2-4 AM) to identify and report the cost of the "phantom load" from standby devices.

> The agent will use a machine learning model (Linear Regression) to predict the total energy usage for a future time day or week  based on past usage patterns.
> Predict the next month's energy consumption and cost based on historical data and provide recommendations for reducing usage during peak hours.

*/


// 1 state iteration (5 seconds) in realtime = 30 minutes in simulation
// daily report interval = 86400 / 1800 = 48 iterations


pub async fn initialize_agent_lookup(mut rx: Receiver<AgentMsg>){
    println!("[AGENT]: Lookup started.......");
    let mut appliance_map: HashMap<String, u32> = HashMap::new();
    // Track currently-on heavy appliances to detect overlapping high-load usage
    let mut heavy_on: HashSet<String> = HashSet::new();
    // Track continuous-on counters per appliance (simulation-step counts)
    let mut duration_map: HashMap<String, u32> = HashMap::new();
    // Track previous wattage reading per appliance to detect spikes
    let mut prev_watts: HashMap<String, f64> = HashMap::new();
    loop{
        tokio::select! {
            update = rx.recv() => {
                match update {
                    Some(msg) => {
                        println!("[AGENT] Received appliance state for ID: {} at sim_time: {}", msg.state.id, msg.sim_time);
                        println!("Update recieved: {:#?}", &msg);
                        //apply_rules(&msg).await;
                        // added rules here ---------------------------------------------------------------------------------
                        println!("[RULES]: Rule engine update recieved.");
                        // alert if bedroom light is on during daylight hours
                        if (&msg.state.name == "Bedroom Light") && *(&msg.state.is_on) {
                            if *&msg.sim_time.hour() >= 6 && *&msg.sim_time.hour() <= 18 {
                                println!("[AGENT ALERT]: Bedroom light is on during daylight hours!");
                            }

                        }

                        // alert if AC is running during off-peak hours
                        if (&msg.state.name == "Air Conditioner") && *(&msg.state.is_on) {
                            if *&msg.sim_time.hour() >= 22 || *&msg.sim_time.hour() < 6 {
                                println!("[AGENT ALERT]: Air Conditioner is running during off-peak hours!");
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
                            println!("[AGENT ALERT]: Peak Load Alert! Total household wattage exceeds 4000W.");
                        }

                        // --- New rules below -------------------------------------------------

                        // Phantom load detection: device is OFF but still drawing small power during deep night
                        if !msg.state.is_on && msg.state.watts > 5.0 {
                            let h = msg.sim_time.hour();
                            if h >= 2 && h <= 4 {
                                println!("[AGENT ALERT]: Possible phantom load: '{}' drawing {:.2}W during {}:00-{}:00", msg.state.name, msg.state.watts, 2, 4);
                            }
                        }

                        // Sudden spike detection: current > 2x previous and absolute delta > 100W
                        if let Some(prev) = prev_watts.get(&msg.state.id) {
                            if msg.state.watts > prev * 2.0 && (msg.state.watts - prev) > 100.0 {
                                println!("[AGENT ALERT]: Sudden wattage spike for '{}': prev={:.2}W curr={:.2}W", msg.state.name, prev, msg.state.watts);
                            }
                        }
                        prev_watts.insert(msg.state.id.clone(), msg.state.watts);

                        // Continuous high-consumption detection: heavy appliance on for long simulated duration
                        if msg.state.is_on {
                            let counter = duration_map.entry(msg.state.id.clone()).or_insert(0);
                            *counter += 1;
                            // Using the same heuristic as microwave: >=2 counts ~ long continuous use in simulation
                            if msg.state.watts > 1000.0 && *counter >= 2 {
                                println!("[AGENT ALERT]: '{}' has been running at high power ({:.2}W) for an extended period.", msg.state.name, msg.state.watts);
                            }
                        } else {
                            duration_map.insert(msg.state.id.clone(), 0);
                        }

                        // Overlapping heavy appliances: track a set of heavy devices that are currently on
                        let heavy_names = ["Air Conditioner", "Washing Machine", "Dryer", "Oven", "Water Heater"];
                        let is_heavy = heavy_names.iter().any(|n| n == &msg.state.name) || msg.state.watts > 800.0;
                        if msg.state.is_on && is_heavy {
                            heavy_on.insert(msg.state.name.clone());
                            if heavy_on.len() > 1 {
                                let appliances: Vec<String> = heavy_on.iter().cloned().collect();
                                println!("[AGENT ALERT]: Multiple high-load appliances running simultaneously: {:?}. Consider staggering start times.", appliances);
                            }
                        } else if !msg.state.is_on {
                            heavy_on.remove(&msg.state.name);
                        }

                        // Suggest scheduling for washing machine / dryer during off-peak
                        if (msg.state.name == "Washing Machine" || msg.state.name == "Dryer") && msg.state.is_on {
                            let h = msg.sim_time.hour();
                            if h >= 7 && h < 22 {
                                println!("[AGENT SUGGESTION]: '{}' is running during peak hours ({}:00). Consider scheduling to off-peak to save cost.", msg.state.name, h);
                            }
                        }

                        // EV Charger / Battery charging suggestion: prefer solar midday (10-16)
                        if msg.state.name.contains("Charger") || msg.state.name.contains("EV") {
                            if msg.state.is_on {
                                let h = msg.sim_time.hour();
                                if h >= 10 && h <= 16 {
                                    println!("[AGENT INFO]: '{}' charging during solar hours ({}:00). Good time to charge.", msg.state.name, h);
                                } else {
                                    println!("[AGENT SUGGESTION]: '{}' is charging outside solar hours ({}:00). Consider shifting to 10:00-16:00 when solar generation is high.", msg.state.name, h);
                                }
                            }
                        }

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