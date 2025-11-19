$root = Split-Path -Parent $PSScriptRoot
$uiDir = Join-Path $root 'web-ui'

Write-Host "Starting Web UI in: $uiDir"
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd '$uiDir'; npm run dev" -WindowStyle Normal
