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
ssh "$($config.user)@$piHost" "rm -f $($config.remote_path)$binaryName"

# Copy binary
Write-Host "Sending binary to Raspberry Pi..." -ForegroundColor Cyan
scp "target/$target/release/$binaryName" "$($config.user)@$piHost`:$($config.remote_path)$binaryName"
if ($LASTEXITCODE -ne 0) { exit 1 }

if ($shouldRun) {
    # Run on Pi
    Write-Host "Running binary on Raspberry Pi..." -ForegroundColor Yellow
    Write-Host "*********************************" -ForegroundColor Yellow
    ssh "$($config.user)@$piHost" "chmod +x $($config.remote_path)$binaryName && sudo $($config.remote_path)$binaryName"
    Write-Host "*********************************" -ForegroundColor Yellow
    Write-Host "Done!" -ForegroundColor Green
} else {
    Write-Host "Upload complete. Skipping run (use -r to autorun)." -ForegroundColor Yellow
}