# Parse Hexapod.toml
$config = @{}
Get-Content Hexapod.toml | ForEach-Object {
    if ($_ -match '^\s*(\w+)\s*=\s*"([^"]+)"') {
        $config[$matches[1]] = $matches[2]
    }
}

# Get binary name from Cargo.toml
$cargoToml = Get-Content Cargo.toml -Raw
$binaryName = if ($cargoToml -match 'name\s*=\s*"([^"]+)"') { $matches[1] } else { "hexapod" }

# Set defaults
$target = if ($config.target) { $config.target } else { "aarch64-unknown-linux-gnu" }
$piHost = if ($args -contains "-l" -or $args -contains "--local") { 
    if ($config.host_local) { $config.host_local } else { "hexapod.local" }
} else { 
    $config.host 
}

# Determine whether to run after upload
$shouldRun = ($args -contains "-r" -or $args -contains "--run")

# Build
Write-Host "Building binary for target '$target'..." -ForegroundColor Cyan
cargo make pibuildrelease
if ($LASTEXITCODE -ne 0) { exit 1 }

# Remove existing binary
Write-Host "Removing existing binary on Raspberry Pi..." -ForegroundColor Cyan
ssh "$($config.user)@$piHost" "sudo pkill hexapod-code; sudo pkill -f 'python3 main.py'"
ssh "$($config.user)@$piHost" "rm -f $($config.remote_path)$binaryName"

# Copy binary
Write-Host "Sending binary to Raspberry Pi..." -ForegroundColor Cyan
scp "target/$target/release/$binaryName" "$($config.user)@$piHost`:$($config.remote_path)$binaryName"
if ($LASTEXITCODE -ne 0) { exit 1 }

# Copy AI module
$aiSourceDir = "crates/devices/src/ai"
if (Test-Path $aiSourceDir) {
    Write-Host "Syncing AI module to Raspberry Pi..." -ForegroundColor Cyan
    # Cleanup pycache before sync to speed up
    Get-ChildItem -Path $aiSourceDir -Recurse -Filter "__pycache__" | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    
    # Force remove remote directory to ensure clean sync and avoid permission issues
    ssh "$($config.user)@$piHost" "sudo rm -rf $($config.remote_path)ai; mkdir -p $($config.remote_path)ai"
    scp -r "$aiSourceDir/*" "$($config.user)@$piHost`:$($config.remote_path)ai/"
    
    # Sync Wheels for offline install
    $wheelsPath = Join-Path (Get-Location) "wheels"
    Write-Host "Checking wheels at: $wheelsPath" -ForegroundColor Magenta
    if (Test-Path $wheelsPath) {
         Write-Host "Syncing offline wheels..." -ForegroundColor Cyan
         ssh "$($config.user)@$piHost" "mkdir -p $($config.remote_path)wheels"
         scp -r "$wheelsPath/*" "$($config.user)@$piHost`:$($config.remote_path)wheels/"
         
         Write-Host "Installing dependencies from offline wheels..." -ForegroundColor Cyan
         # Try to uninstall requests first to clear bad state
         ssh "$($config.user)@$piHost" "sudo pip3 uninstall -y --break-system-packages requests urllib3 charset-normalizer"
         ssh "$($config.user)@$piHost" "cd $($config.remote_path)ai && sudo -H pip3 install --break-system-packages --force-reinstall --no-index --find-links ../wheels fastapi uvicorn starlette typing_extensions pydantic annotated-doc anyio idna sniffio click colorama requests urllib3 charset_normalizer certifi openai distro tqdm networkx numpy websockets"
    } else {
         Write-Host "Wheels directory not found!" -ForegroundColor Red
    }
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Warning: Failed to sync/install AI module" -ForegroundColor Yellow
    } else {
        Write-Host "AI module synced and installed." -ForegroundColor Green
    }
}

if ($shouldRun) {
    # Run on Pi
    Write-Host "Running binary on Raspberry Pi..." -ForegroundColor Yellow
    Write-Host "*********************************" -ForegroundColor Yellow
    ssh "$($config.user)@$piHost" "chmod +x $($config.remote_path)$binaryName && sudo -E $($config.remote_path)$binaryName"
    Write-Host "*********************************" -ForegroundColor Yellow
    Write-Host "Done!" -ForegroundColor Green
} else {
    Write-Host "Upload complete. Skipping run (use -r to autorun)." -ForegroundColor Yellow
}