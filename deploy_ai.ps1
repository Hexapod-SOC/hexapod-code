# Parse Hexapod.toml
$config = @{}
Get-Content Hexapod.toml | ForEach-Object {
    if ($_ -match '^\s*(\w+)\s*=\s*"([^"]+)"') {
        $config[$matches[1]] = $matches[2]
    }
}

# Defaults
$piHost = if ($args -contains "-l" -or $args -contains "--local") { 
    if ($config.host_local) { $config.host_local } else { "hexapod.local" }
} else { 
    $config.host 
}

$remotePath = $config.remote_path
$binaryName = "hexapod-code" # Assumption

Write-Host "Deploying AI module to $piHost..." -ForegroundColor Cyan

# Copy AI module
$aiSourceDir = "ai"
if (Test-Path $aiSourceDir) {
    Write-Host "Syncing AI module..." -ForegroundColor Cyan
    # Cleanup pycache
    Get-ChildItem -Path $aiSourceDir -Recurse -Filter "__pycache__" | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    
    # Clean remote to avoid stale files
    ssh "$($config.user)@$piHost" "sudo rm -rf $($remotePath)ai; mkdir -p $($remotePath)ai"
    if ($LASTEXITCODE -ne 0) { Write-Host "Failed to clean remote dir" -ForegroundColor Red; exit 1 }

    # SCP
    scp -r "$aiSourceDir/*" "$($config.user)@$piHost`:$($remotePath)ai/"
    if ($LASTEXITCODE -ne 0) { Write-Host "Failed to scp files" -ForegroundColor Red; exit 1 }
    
    Write-Host "AI module synced." -ForegroundColor Green
    
    # Restart service if -r provided
    if ($args -contains "-r") {
        Write-Host "Restarting Hexapod Service..." -ForegroundColor Yellow
        # Kill existing
        ssh "$($config.user)@$piHost" "sudo pkill $binaryName; sudo pkill -f 'python3 main.py'"
        
        # Start
        Write-Host "Starting binary..." -ForegroundColor Yellow
        ssh "$($config.user)@$piHost" "chmod +x $($remotePath)$binaryName && sudo -E $($remotePath)$binaryName"
    }
} else {
    Write-Host "AI source directory not found!" -ForegroundColor Red
    exit 1
}
