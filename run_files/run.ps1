
Write-Host "Launching AETHER Agent, Home Simulator, and Web UI..."

$root = Split-Path -Parent $PSScriptRoot

# Paths
$agentDir = Join-Path $root 'aether_agent'
$simDir = Join-Path $root 'home_simulator'
$uiDir = Join-Path $root 'web-ui'

Write-Host "Agent dir: $agentDir"
Write-Host "Simulator dir: $simDir"
Write-Host "Web UI dir: $uiDir"

# Start processes
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd '$agentDir'; cargo run" -WindowStyle Normal
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd '$simDir'; cargo run" -WindowStyle Normal
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd '$uiDir'; npm run dev" -WindowStyle Normal

Write-Host "Launched all processes. Check the opened windows for logs."
