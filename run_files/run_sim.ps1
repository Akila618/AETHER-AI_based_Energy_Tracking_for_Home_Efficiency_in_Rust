(The file `c:\Users\User\Desktop\AI MP\AETHER-AI_based_Energy_Tracking_for_Home_Efficiency_in_Rust\run_files\run_sim.ps1` is being created)
$root = Split-Path -Parent $PSScriptRoot
$simDir = Join-Path $root 'home_simulator'

Write-Host "Starting Home Simulator in: $simDir"
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd '$simDir'; cargo run" -WindowStyle Normal
