
Write-Host "Launching AETHER Agent, Home Simulator, and Web UI..."

$root = Split-Path -Parent $PSScriptRoot

# Paths
$agentDir = Join-Path $root 'aether_agent'
$simDir = Join-Path $root 'home_simulator'
$uiDir = Join-Path $root 'web-ui'

Write-Host "Agent dir: $agentDir"
Write-Host "Simulator dir: $simDir"
Write-Host "Web UI dir: $uiDir"

function Start-ProcessWindow($label, $path, $command) {
	Write-Host "Starting $label in: $path"
	Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd '$path'; $command" -WindowStyle Normal
}

function Wait-ForPort($host, $port, $timeoutSec) {
	$end = (Get-Date).AddSeconds($timeoutSec)
	while ((Get-Date) -lt $end) {
		try {
			$tcp = New-Object System.Net.Sockets.TcpClient
			$iar = $tcp.BeginConnect($host, $port, $null, $null)
			$ok = $iar.AsyncWaitHandle.WaitOne(500)
			if ($ok -and $tcp.Connected) {
				$tcp.EndConnect($iar)
				$tcp.Close()
				return $true
			}
			$tcp.Close()
		} catch {
			# ignore
		}
		Start-Sleep -Seconds 1
	}
	return $false
}

# Start the agent first
Start-ProcessWindow 'AETHER Agent' $agentDir 'cargo run'

# Wait for the agent to start listening on port 3000 before launching the simulator
Write-Host "Waiting up to 30s for agent to listen on 127.0.0.1:3000..."
if (Wait-ForPort '127.0.0.1' 3000 30) {
	Write-Host "Agent appears to be listening."
} else {
	Write-Warning "Agent did not respond within 30s. Starting simulator anyway (it will retry connecting)."
}

# Start the simulator
Start-ProcessWindow 'Home Simulator' $simDir 'cargo run'

# For the UI, ensure dependencies are installed; if node_modules not present, run npm install first
if (-not (Test-Path (Join-Path $uiDir 'node_modules'))) {
	Write-Host "node_modules not found in web-ui; running 'npm install' and then 'npm run dev'"
	Start-Process pwsh -ArgumentList "-NoExit", "-Command", "cd '$uiDir'; npm install; npm run dev" -WindowStyle Normal
} else {
	Start-ProcessWindow 'Web UI' $uiDir 'npm run dev'
}

Write-Host "Launched all processes. Check the opened windows for logs."
