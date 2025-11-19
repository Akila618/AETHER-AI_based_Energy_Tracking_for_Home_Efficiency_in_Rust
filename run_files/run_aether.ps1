$root = Split-Path -Parent $PSScriptRoot
$agentDir = Join-Path $root 'aether_agent'

Write-Host "Starting AETHER agent in: $agentDir"

Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd '$agentDir'; cargo run" -WindowStyle Normal
