use linfa::prelude::*;
use ndarray::{Array1, Array2};
use linfa_linear::{FittedIsotonicRegression, FittedLinearRegression, LinearRegression};
use sqlx::{MySqlPool, pool, Row};
use chrono::{NaiveDate, Utc, Datelike, Duration};
//use supervised: linear regression model

/* 

> The agent will use a machine learning model (Linear Regression) to predict the total energy usage for a future time day or week  based on past usage patterns.
> Predict the next month's energy consumption and cost based on historical data and provide recommendations for reducing usage during peak hours.   

dataframe with historical data of appliance states, timestamps, and energy consumption (day, number of appliances on(x), total watts consumed(y))

linear regression finds the best fit line through the data points to predict future energy consumption based on number of appliances on and time of day.

*/

use linfa::{
    traits::{Fit, Predict},
    DatasetBase,
    Error,
};
use serde_json::json;

const LKR_PER_KWH: f64 = 3.0; 
const SNAPSHOTS_PER_DAY: f64 = 48.0; 
const SNAPSHOT_HOURS: f64 = 0.5; 


// Use Box<dyn std::error::Error> for simplified error handling
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// We use i64 for count fields as they come from the database
#[derive(Debug, sqlx::FromRow)]
pub struct SnapshotData {
    pub simulated_date: NaiveDate,
    pub appliances_used_count: i64,
    pub total_daily_watts: f64,
}

pub struct TrainResult {
    pub model: FittedLinearRegression<f64>,
    pub mean: f64,
    pub std: f64,
    pub avg_target: f64,
}

pub async fn train_model(pool: &MySqlPool) -> Result<TrainResult> {
    println!("[MODEL TRAINING]: Training energy consumption prediction model...");
    
    // simulated_date and the total_daily_watts for forecasting based on date
    let history_data: Vec<SnapshotData> = sqlx::query_as::<_, SnapshotData>(
        "
        SELECT
            DATE(sim_timestamp) AS simulated_date,
            CAST(COUNT(DISTINCT CASE WHEN is_on = TRUE THEN name ELSE NULL END) AS SIGNED) AS appliances_used_count,
            CAST(ROUND(SUM(CASE WHEN is_on = TRUE THEN watts ELSE 0 END), 2) AS DOUBLE) AS total_daily_watts
        FROM home_state
        WHERE
            sim_timestamp >= (
                -- Use 60 simulated days of history for training
                SELECT DATE_SUB(MAX(sim_timestamp), INTERVAL 100 DAY)
                FROM home_state
            )
        GROUP BY simulated_date
        ORDER BY simulated_date DESC;
        "
    )
    .fetch_all(pool)
    .await?;

    if history_data.len() < 50 {
        return Err("Not enough historical data (less than 50 days) to train the model.".into());
    }

    // convert date to numeric format
    let n_samples = history_data.len();
    let n_features = 1;
    
    let mut features: Vec<f64> = Vec::with_capacity(n_samples * n_features);
    let mut targets: Vec<f64> = Vec::with_capacity(n_samples);
    
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    
    for d in &history_data {
        // Feature 1 (X): Days since epoch (numerical representation of the date)
        let date_num = d.simulated_date.signed_duration_since(epoch).num_days() as f64;
        features.push(date_num);

        // Target (Y): Total watts consumption
        targets.push(d.total_daily_watts);
    }

    println!("[DEBUG] N_SAMPLES: {}", n_samples);
    println!("[DEBUG] First 5 Features (DateNum): {:?}", &features[0..5]);
    println!("[DEBUG] First 5 Targets (Watts): {:?}", &targets[0..5]);

    let target_sum: f64 = targets.iter().sum();
    if target_sum < 1.0 {
        println!("[DEBUG] TARGET SUM IS ZERO. Check simulator or database insertion logic.");
        return Err("Target data (Total Daily Watts) is zero.".into());
    }

    // standardize features to improve numerical stability
    let mean = features.iter().sum::<f64>() / (n_samples as f64);
    let variance = features.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n_samples as f64);
    let std = variance.sqrt().max(1e-8);
    let standardized: Vec<f64> = features.iter().map(|v| (v - mean) / std).collect();

    // create the input feature array (X) and target array (Y)
    let x_arr = Array2::from_shape_vec((n_samples, n_features), standardized.clone()).unwrap();
    let y_arr = Array1::from_vec(targets.clone());
    let dataset = DatasetBase::new(x_arr.clone(), y_arr.clone());

    // train the model
    let model = LinearRegression::default()
        .fit(&dataset)
        .expect("Failed to train linear regression model.");

    // predict on training data to inspect fit quality
    let train_preds = model.predict(&x_arr);
    println!("[MODEL DEBUG] First 5 train predictions: {:?}", &train_preds.as_slice().unwrap()[0..5.min(train_preds.len())]);
    println!("[MODEL DEBUG] First 5 train targets: {:?}", &y_arr.as_slice().unwrap()[0..5.min(y_arr.len())]);

    let avg_target = target_sum / (n_samples as f64);
    println!("[MODEL TRAINING]: Training successful. Model ready. mean={}, std={}, avg_target={}", mean, std, avg_target);
    Ok(TrainResult { model, mean, std, avg_target })
}

// function to get the number of days in a target month/year
fn days_in_month(year: i32, month: u32) -> u32 {
    if month == 2 {
        // Check for leap year
        if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) {
            return 29;
        } else {
            return 28;
        }
    } else if [4, 6, 9, 11].contains(&month) {
        return 30;
    } else {
        return 31;
    }
}


// Predicts the total wattage consumption for a specific month (year, month)
pub async fn run_prediction_for_month(pool: &MySqlPool, year: i32, month: u32) -> Result<String> {
    //get model and normalization stats)
    let train = train_model(pool).await?;
    let model = train.model;
    let mean = train.mean;
    let std = train.std;

    // determine the range and features for prediction
    let num_days = days_in_month(year, month);
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let mut total_predicted_watts = 0.0;

    let mut prediction_features: Vec<f64> = Vec::new();

    for day in 1..=num_days {
        let date = NaiveDate::from_ymd_opt(year, month, day).ok_or("Invalid date")?;
        let date_num = date.signed_duration_since(epoch).num_days() as f64;
        prediction_features.push((date_num - mean) / std);
    }

    let prediction_array = Array2::from_shape_vec((num_days as usize, 1), prediction_features).unwrap();
    let predictions = model.predict(&prediction_array);
    println!("[PREDICT] Year: {}, Month: {}, NumDays: {}", year, month, num_days);
    if num_days > 0 {
        let first_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        let last_date = NaiveDate::from_ymd_opt(year, month, num_days).unwrap();
        println!("[PREDICT] Date Range: {} to {}", first_date, last_date);
    }

    // aggregate the results (and print per-day values for debugging)
    for i in 0..num_days as usize {
        let raw_daily = predictions[i];
        let daily_watts = raw_daily.max(0.0);
        let day = i + 1;
        println!("[PREDICT] Day {}-{}-{} -> raw={:.6}W clipped={:.2}W", year, month, day, raw_daily, daily_watts);
        total_predicted_watts += daily_watts;
    }
    if total_predicted_watts < 1.0 {
        println!("[PREDICT] Model produced non-positive predictions; falling back to historical average target.");
        let fallback_daily = train.avg_target;
        total_predicted_watts = fallback_daily * (num_days as f64);
    }

    let total_simulated_hours = num_days as f64 * 24.0;
    let total_monthly_kwh = (total_predicted_watts / SNAPSHOTS_PER_DAY) / 1000.0 * total_simulated_hours;
    let total_monthly_cost = total_monthly_kwh * LKR_PER_KWH;

    Ok(format!(
        "[PREDICTION] Estimated Usage for {}-{}: {:.2} kWh. Estimated Cost: LKR {:.2}",
        year, month,
        total_monthly_kwh,
        total_monthly_cost
    ))

}

/// Return a structured prediction as JSON value with numeric fields.


/// Return a structured prediction as JSON value with numeric fields.
pub async fn run_prediction_for_month_json(pool: &MySqlPool, year: i32, month: u32) -> Result<serde_json::Value> {
    let train = train_model(pool).await?;
    let model = train.model;
    let mean = train.mean;
    let std = train.std;

    let num_days = days_in_month(year, month);
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();

    let mut prediction_features: Vec<f64> = Vec::new();
    for day in 1..=num_days {
        let date = NaiveDate::from_ymd_opt(year, month, day).ok_or("Invalid date")?;
        let date_num = date.signed_duration_since(epoch).num_days() as f64;
        prediction_features.push((date_num - mean) / std);
    }

    let prediction_array = Array2::from_shape_vec((num_days as usize, 1), prediction_features).unwrap();
    let predictions = model.predict(&prediction_array);

    let mut total_predicted_watts = 0.0;
    let mut daily_watts: Vec<f64> = Vec::new();
    for i in 0..num_days as usize {
        let raw_daily = predictions[i];
        let daily_w = raw_daily.max(0.0);
        daily_watts.push(daily_w);
        total_predicted_watts += daily_w;
    }

    if total_predicted_watts < 1.0 {
        let fallback_daily = train.avg_target;
        total_predicted_watts = fallback_daily * (num_days as f64);
        daily_watts = vec![fallback_daily; num_days as usize];
    }

    let total_simulated_hours = num_days as f64 * 24.0;
    let total_monthly_kwh = (total_predicted_watts / SNAPSHOTS_PER_DAY) / 1000.0 * total_simulated_hours;
    let total_monthly_cost = total_monthly_kwh * LKR_PER_KWH;

    Ok(json!({
        "year": year,
        "month": month,
        "monthly_kwh": total_monthly_kwh,
        "monthly_cost": total_monthly_cost,
        "daily_watts": daily_watts
    }))
}
    
