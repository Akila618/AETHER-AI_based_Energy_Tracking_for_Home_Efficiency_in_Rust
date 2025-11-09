use tokio::time::{sleep, Duration};
use tokio::sync::mpsc::Receiver;
use aether_utils::AgentMsg;
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

- * This is our advanced feature, which also runs on the periodic timer: 
> The agent will use a machine learning model (Linear Regression) to predict the total energy usage for a future time day or week  based on past usage patterns.
> Predict the next month's energy consumption and cost based on historical data and provide recommendations for reducing usage during peak hours.

*/

pub async fn initialize_agent_lookup(mut rx: Receiver<AgentMsg>){
    println!("[AGENT]: Lookup started.......");
    loop{
        tokio::select! {
            maybe_msg = rx.recv() => {
                match maybe_msg {
                    Some(msg) => println!("[AGENT] Received message for lookup: {:?}", msg),
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

#[allow(dead_code)]
async fn apply_rules(_rx: Receiver<AgentMsg>){
    // implement rule application here
}