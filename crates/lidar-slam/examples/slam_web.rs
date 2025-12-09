//! SLAM Web Visualization Example
//!
//! This example runs SLAM on LiDAR data and provides a web interface
//! to visualize the occupancy grid map, robot trajectory, and real-time scans.
//!
//! # Usage
//!
//! On Raspberry Pi with real hardware:
//! ```bash
//! cargo run --example slam_web --features real -p lidar-slam
//! ```
//!
//! For testing on PC (dummy mode):
//! ```bash
//! cargo run --example slam_web --features dummy -p lidar-slam
//! ```
//!
//! Then open in browser: http://localhost:3002

use devices::lidar::LidarDriver;
use lidar_slam::{SlamBuilder, SlamProcessor, Scan2D, Point2D};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use tower_http::cors::{Any, CorsLayer};

type SharedSlam = Arc<Mutex<SlamProcessor>>;
type SharedScan = Arc<Mutex<Option<Vec<Point2D>>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          Hexapod SLAM Web Visualization                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let port = "/dev/ttyUSB0";
    
    println!("🔌 Connecting to LiDAR on port: {}", port);
    let driver = match LidarDriver::new(port) {
        Ok(mut d) => {
            println!("✓ Successfully opened serial port");
            println!("🚀 Starting LiDAR...");
            d.start()?;
            println!("✓ LiDAR is running");
            Arc::new(std::sync::Mutex::new(d))
        }
        Err(e) => {
            eprintln!("✗ Failed to open serial port: {}", e);
            return Err(e);
        }
    };

    // Create SLAM processor
    println!("📊 Initializing SLAM processor...");
    let slam = SlamBuilder::new()
        .with_grid_resolution(50.0)        // 5cm cells
        .with_grid_size(400, 400)          // 20m x 20m map
        .with_max_range(8000.0)            // 8m max range
        .with_gyro(false)                  // Gyro disconnected
        .with_lidar_height(100.0)          // 10cm from ground
        .build();
    
    let slam = Arc::new(Mutex::new(slam));
    let current_scan: SharedScan = Arc::new(Mutex::new(None));
    println!("✓ SLAM initialized");

    // Create broadcast channel for WebSocket updates
    let (tx, _rx) = broadcast::channel::<String>(100);

    // Spawn SLAM processing thread
    let slam_clone = Arc::clone(&slam);
    let driver_clone = Arc::clone(&driver);
    let tx_clone = tx.clone();
    let scan_clone = Arc::clone(&current_scan);
    
    thread::spawn(move || {
        slam_processing_loop(driver_clone, slam_clone, tx_clone, scan_clone);
    });

    // Build router
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(websocket_handler))
        .layer(cors)
        .with_state((slam, tx, current_scan));

    let addr = "0.0.0.0:3002";
    
    println!();
    println!("🌐 Web server starting on {}", addr);
    println!("📱 Open in browser: http://localhost:3002");
    println!("   (or http://YOUR_PI_IP:3002 from another device)");
    println!();
    println!("Press Ctrl+C to stop");
    println!();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn slam_processing_loop(
    driver: Arc<std::sync::Mutex<LidarDriver>>,
    slam: Arc<Mutex<SlamProcessor>>,
    tx: broadcast::Sender<String>,
    current_scan: SharedScan,
) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let mut last_scan_time = Instant::now();
    let mut last_frame_time = Instant::now();
    let mut last_map_time = Instant::now();
    let mut scan_count = 0u64;
    let mut no_data_count = 0u64;
    
    println!("🔄 SLAM processing loop started");
    
    loop {
        // Check if frame is ready
        let frame_ready = {
            let driver = driver.lock().unwrap();
            driver.is_frame_ready()
        };
        
        if !frame_ready {
            no_data_count += 1;
            if no_data_count % 500 == 0 {
                let driver = driver.lock().unwrap();
                println!("⏳ Waiting for LiDAR data... (speed: {:.1} Hz, errors: {})", 
                    driver.get_speed(), driver.get_error_count());
            }
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        
        // Get point cloud from LiDAR
        let cloud = {
            let driver = driver.lock().unwrap();
            driver.get_point_cloud()
        };

        if let Some(cloud) = cloud {
            no_data_count = 0;
            let point_count = cloud.valid_count();
            
            if point_count < 10 {
                // Not enough points, skip
                continue;
            }
            
            // Convert to SLAM scan
            let scan = Scan2D::from_point_cloud(&cloud);
            
            // Process with SLAM
            let world_points = rt.block_on(async {
                let mut slam = slam.lock().await;
                slam.process_scan_2d(&scan);
                
                // Get world-transformed scan points for visualization
                let pose = slam.current_pose();
                scan.points.iter().map(|p| pose.transform_point(p)).collect::<Vec<_>>()
            });

            // Store current scan for WebSocket
            rt.block_on(async {
                let mut scan_lock = current_scan.lock().await;
                *scan_lock = Some(world_points.clone());
            });

            scan_count += 1;
            
            // Send updates every 100ms for pose/scan
            if last_frame_time.elapsed() >= Duration::from_millis(100) {
                last_frame_time = Instant::now();
                
                // Check if we should include map (every 2 seconds)
                let include_map = last_map_time.elapsed() >= Duration::from_secs(2);
                if include_map {
                    last_map_time = Instant::now();
                }
                
                // Build update JSON
                let update = rt.block_on(async {
                    let slam = slam.lock().await;
                    
                    let mut json_obj = serde_json::json!({
                        "pose": {
                            "x": slam.current_pose().x,
                            "y": slam.current_pose().y,
                            "theta": slam.current_pose().theta,
                        },
                        "scan": world_points.iter().take(200).map(|p| {
                            serde_json::json!({"x": p.x, "y": p.y})
                        }).collect::<Vec<_>>(),
                        "trajectory": slam.trajectory().iter().map(|p| {
                            serde_json::json!({"x": p.x, "y": p.y, "theta": p.theta})
                        }).collect::<Vec<_>>(),
                        "stats": {
                            "scan_count": slam.scan_count(),
                            "match_quality": slam.get_state().match_quality,
                            "is_initialized": slam.is_initialized(),
                            "point_count": point_count,
                        }
                    });
                    
                    // Only include map periodically to reduce bandwidth
                    if include_map {
                        let map = slam.get_map();
                        let map_data = map.to_image_data();
                        json_obj["map"] = serde_json::json!({
                            "width": map.dimensions().0,
                            "height": map.dimensions().1,
                            "resolution": map.resolution(),
                            "origin_x": map.origin().x,
                            "origin_y": map.origin().y,
                            "cells": map_data,
                        });
                    }
                    
                    json_obj
                });

                if let Ok(json) = serde_json::to_string(&update) {
                    let _ = tx.send(json);
                }
            }

            // Print stats periodically
            if scan_count % 50 == 0 {
                let elapsed = last_scan_time.elapsed();
                let fps = 50.0 / elapsed.as_secs_f32();
                last_scan_time = Instant::now();
                
                rt.block_on(async {
                    let slam = slam.lock().await;
                    let pose = slam.current_pose();
                    let map = slam.get_map();
                    
                    // Count occupied and free cells
                    let map_data = map.to_image_data();
                    let occupied = map_data.iter().filter(|&&v| v < 100).count();
                    let free = map_data.iter().filter(|&&v| v > 160).count();
                    
                    println!(
                        "📍 Pose: ({:.0}, {:.0}, {:.1}°) | Scans: {} | Points: {} | FPS: {:.1} | Map: {} occupied, {} free",
                        pose.x,
                        pose.y,
                        pose.theta.to_degrees(),
                        slam.scan_count(),
                        point_count,
                        fps,
                        occupied,
                        free
                    );
                });
            }
        }
        
        // Small sleep to prevent busy loop
        thread::sleep(Duration::from_millis(1));
    }
}

type AppState = (SharedSlam, broadcast::Sender<String>, SharedScan);

async fn index_handler() -> Html<&'static str> {
    Html(SLAM_HTML)
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State((slam, tx, _scan)): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, slam, tx))
}

async fn handle_websocket(
    mut socket: WebSocket,
    slam: SharedSlam,
    tx: broadcast::Sender<String>,
) {
    let mut rx = tx.subscribe();
    
    println!("🌐 WebSocket client connected");

    // Send initial state
    {
        let slam = slam.lock().await;
        let initial = serde_json::json!({
            "type": "init",
            "pose": {
                "x": slam.current_pose().x,
                "y": slam.current_pose().y,
                "theta": slam.current_pose().theta,
            },
            "stats": {
                "scan_count": slam.scan_count(),
                "is_initialized": slam.is_initialized(),
            }
        });
        if let Ok(json) = serde_json::to_string(&initial) {
            let _ = socket.send(Message::Text(json.into())).await;
        }
    }

    let mut ping_interval = tokio::time::interval(Duration::from_secs(5));

    // Forward updates to client
    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                // Send ping to keep connection alive
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            msg = rx.recv() => {
                match msg {
                    Ok(data) => {
                        if socket.send(Message::Text(data.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Skip lagged messages
                        continue;
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Pong(_))) => {
                        // Pong received, connection is alive
                    }
                    _ => {}
                }
            }
        }
    }
    
    println!("🌐 WebSocket client disconnected");
}

const SLAM_HTML: &str = r##"<!DOCTYPE html>
<html>
<head>
    <title>Hexapod SLAM</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0d1117; 
            color: #c9d1d9;
            display: flex;
            height: 100vh;
            overflow: hidden;
        }
        
        /* Sidebar */
        #sidebar {
            width: 300px;
            background: #161b22;
            border-right: 1px solid #30363d;
            display: flex;
            flex-direction: column;
        }
        
        #header {
            padding: 20px;
            border-bottom: 1px solid #30363d;
        }
        
        #header h1 {
            font-size: 1.5em;
            color: #58a6ff;
            display: flex;
            align-items: center;
            gap: 10px;
        }
        
        #status-bar {
            display: flex;
            align-items: center;
            gap: 8px;
            margin-top: 12px;
            padding: 8px 12px;
            border-radius: 6px;
            font-size: 0.85em;
        }
        
        .status-dot {
            width: 10px;
            height: 10px;
            border-radius: 50%;
            animation: pulse 2s infinite;
        }
        
        .connected .status-dot { background: #3fb950; }
        .disconnected .status-dot { background: #f85149; animation: none; }
        .connected { background: rgba(63, 185, 80, 0.1); color: #3fb950; }
        .disconnected { background: rgba(248, 81, 73, 0.1); color: #f85149; }
        
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        
        #stats {
            flex: 1;
            padding: 20px;
            overflow-y: auto;
        }
        
        .stat-section {
            margin-bottom: 24px;
        }
        
        .stat-section h3 {
            font-size: 0.75em;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            color: #8b949e;
            margin-bottom: 12px;
            display: flex;
            align-items: center;
            gap: 8px;
        }
        
        .stat-row {
            display: flex;
            justify-content: space-between;
            padding: 8px 0;
            border-bottom: 1px solid #21262d;
        }
        
        .stat-label { color: #8b949e; }
        .stat-value { 
            font-family: 'SF Mono', 'Fira Code', monospace;
            color: #58a6ff;
            font-weight: 500;
        }
        
        /* Quality bar */
        .quality-bar {
            height: 4px;
            background: #21262d;
            border-radius: 2px;
            margin-top: 8px;
            overflow: hidden;
        }
        
        .quality-fill {
            height: 100%;
            background: linear-gradient(90deg, #f85149, #d29922, #3fb950);
            transition: width 0.3s;
        }
        
        /* Mini map */
        #minimap-container {
            padding: 20px;
            border-top: 1px solid #30363d;
        }
        
        #minimap {
            width: 100%;
            height: 150px;
            background: #0d1117;
            border-radius: 8px;
            border: 1px solid #30363d;
        }
        
        /* Main area */
        #main {
            flex: 1;
            display: flex;
            flex-direction: column;
            position: relative;
        }
        
        #canvas-container {
            flex: 1;
            position: relative;
            background: #0d1117;
        }
        
        canvas {
            position: absolute;
            top: 0;
            left: 0;
        }
        
        /* Overlay info */
        #overlay-info {
            position: absolute;
            top: 20px;
            left: 20px;
            background: rgba(22, 27, 34, 0.9);
            padding: 12px 16px;
            border-radius: 8px;
            border: 1px solid #30363d;
            font-family: 'SF Mono', monospace;
            font-size: 0.85em;
        }
        
        #zoom-level {
            color: #58a6ff;
        }
        
        /* Controls */
        #controls {
            position: absolute;
            bottom: 20px;
            left: 50%;
            transform: translateX(-50%);
            background: rgba(22, 27, 34, 0.95);
            padding: 12px 20px;
            border-radius: 12px;
            border: 1px solid #30363d;
            display: flex;
            gap: 16px;
            align-items: center;
        }
        
        .btn {
            background: #21262d;
            border: 1px solid #30363d;
            color: #c9d1d9;
            padding: 8px 16px;
            border-radius: 6px;
            cursor: pointer;
            font-size: 0.85em;
            transition: all 0.2s;
        }
        
        .btn:hover {
            background: #30363d;
            border-color: #8b949e;
        }
        
        .btn-primary {
            background: #238636;
            border-color: #238636;
            color: white;
        }
        
        .btn-primary:hover {
            background: #2ea043;
        }
        
        .toggle {
            display: flex;
            align-items: center;
            gap: 6px;
            cursor: pointer;
            user-select: none;
        }
        
        .toggle input {
            width: 16px;
            height: 16px;
            accent-color: #58a6ff;
        }
        
        .divider {
            width: 1px;
            height: 24px;
            background: #30363d;
        }
        
        /* Coordinate display */
        #coords {
            position: absolute;
            bottom: 80px;
            left: 20px;
            background: rgba(22, 27, 34, 0.9);
            padding: 8px 12px;
            border-radius: 6px;
            font-family: 'SF Mono', monospace;
            font-size: 0.8em;
            color: #8b949e;
        }
    </style>
</head>
<body>
    <div id="sidebar">
        <div id="header">
            <h1>🤖 Hexapod SLAM</h1>
            <div id="status-bar" class="disconnected">
                <div class="status-dot"></div>
                <span id="status-text">Connecting...</span>
            </div>
        </div>
        
        <div id="stats">
            <div class="stat-section">
                <h3>📍 Position</h3>
                <div class="stat-row">
                    <span class="stat-label">X</span>
                    <span class="stat-value"><span id="pose-x">0.00</span> m</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Y</span>
                    <span class="stat-value"><span id="pose-y">0.00</span> m</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Heading</span>
                    <span class="stat-value"><span id="pose-theta">0.0</span>°</span>
                </div>
            </div>
            
            <div class="stat-section">
                <h3>📊 Mapping</h3>
                <div class="stat-row">
                    <span class="stat-label">Scans processed</span>
                    <span class="stat-value" id="scan-count">0</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Current points</span>
                    <span class="stat-value" id="point-count">0</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Mapped cells</span>
                    <span class="stat-value" id="map-cells">0</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Match quality</span>
                    <span class="stat-value" id="quality">--</span>
                </div>
                <div class="quality-bar">
                    <div class="quality-fill" id="quality-bar" style="width: 0%"></div>
                </div>
            </div>
            
            <div class="stat-section">
                <h3>⚡ Performance</h3>
                <div class="stat-row">
                    <span class="stat-label">Update rate</span>
                    <span class="stat-value"><span id="fps">0</span> Hz</span>
                </div>
                <div class="stat-row">
                    <span class="stat-label">Messages</span>
                    <span class="stat-value" id="update-count">0</span>
                </div>
            </div>
        </div>
        
        <div id="minimap-container">
            <canvas id="minimap"></canvas>
        </div>
    </div>
    
    <div id="main">
        <div id="canvas-container">
            <canvas id="mapCanvas"></canvas>
            <canvas id="overlayCanvas"></canvas>
        </div>
        
        <div id="overlay-info">
            Zoom: <span id="zoom-level">100%</span>
        </div>
        
        <div id="coords">
            Mouse: <span id="mouse-coords">--, --</span>
        </div>
        
        <div id="controls">
            <button class="btn btn-primary" onclick="centerOnRobot()">📍 Center</button>
            <button class="btn" onclick="resetView()">↺ Reset</button>
            <button class="btn" onclick="zoomIn()">+</button>
            <button class="btn" onclick="zoomOut()">−</button>
            <div class="divider"></div>
            <label class="toggle"><input type="checkbox" id="showMap" checked> Map</label>
            <label class="toggle"><input type="checkbox" id="showScan" checked> Scan</label>
            <label class="toggle"><input type="checkbox" id="showTrajectory" checked> Path</label>
            <label class="toggle"><input type="checkbox" id="autoCenter"> Follow</label>
        </div>
    </div>

<script>
const mapCanvas = document.getElementById('mapCanvas');
const overlayCanvas = document.getElementById('overlayCanvas');
const minimap = document.getElementById('minimap');
const mapCtx = mapCanvas.getContext('2d');
const overlayCtx = overlayCanvas.getContext('2d');
const minimapCtx = minimap.getContext('2d');

let viewOffset = { x: 0, y: 0 };
let viewScale = 0.1;
let robotPose = { x: 0, y: 0, theta: 0 };
let trajectory = [];
let currentScan = [];
let mapData = null;
let updateCount = 0;
let lastFrameTime = Date.now();
let frameCount = 0;
let matchQuality = 0;

function resizeCanvas() {
    const container = document.getElementById('canvas-container');
    const w = container.clientWidth;
    const h = container.clientHeight;
    mapCanvas.width = overlayCanvas.width = w;
    mapCanvas.height = overlayCanvas.height = h;
    viewOffset = { x: w/2, y: h/2 };
    
    const mm = document.getElementById('minimap-container');
    minimap.width = mm.clientWidth - 40;
    minimap.height = 150;
    
    render();
}
window.addEventListener('resize', resizeCanvas);
resizeCanvas();

// Pan and zoom
let isDragging = false;
let lastMouse = { x: 0, y: 0 };

overlayCanvas.addEventListener('mousedown', e => {
    isDragging = true;
    lastMouse = { x: e.clientX, y: e.clientY };
    overlayCanvas.style.cursor = 'grabbing';
});

overlayCanvas.addEventListener('mousemove', e => {
    // Update coordinates display
    const rect = overlayCanvas.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    const wx = (sx - viewOffset.x) / viewScale;
    const wy = -(sy - viewOffset.y) / viewScale;
    document.getElementById('mouse-coords').textContent = 
        `${(wx/1000).toFixed(2)}m, ${(wy/1000).toFixed(2)}m`;
    
    if (isDragging) {
        viewOffset.x += e.clientX - lastMouse.x;
        viewOffset.y += e.clientY - lastMouse.y;
        lastMouse = { x: e.clientX, y: e.clientY };
        render();
    }
});

overlayCanvas.addEventListener('mouseup', () => {
    isDragging = false;
    overlayCanvas.style.cursor = 'default';
});
overlayCanvas.addEventListener('mouseleave', () => {
    isDragging = false;
    overlayCanvas.style.cursor = 'default';
});

overlayCanvas.addEventListener('wheel', e => {
    e.preventDefault();
    const zoom = e.deltaY > 0 ? 0.9 : 1.1;
    viewScale *= zoom;
    viewScale = Math.max(0.02, Math.min(0.5, viewScale));
    document.getElementById('zoom-level').textContent = Math.round(viewScale * 1000) + '%';
    render();
});

function worldToScreen(x, y) {
    return {
        x: viewOffset.x + x * viewScale,
        y: viewOffset.y - y * viewScale
    };
}

function render() {
    const w = overlayCanvas.width;
    const h = overlayCanvas.height;
    
    // Clear
    mapCtx.fillStyle = '#0d1117';
    mapCtx.fillRect(0, 0, w, h);
    overlayCtx.clearRect(0, 0, w, h);
    
    // Draw grid
    const gridSize = 1000 * viewScale;
    if (gridSize > 10) {
        overlayCtx.strokeStyle = '#21262d';
        overlayCtx.lineWidth = 1;
        const startX = viewOffset.x % gridSize;
        const startY = viewOffset.y % gridSize;
        
        for (let x = startX; x < w; x += gridSize) {
            overlayCtx.beginPath();
            overlayCtx.moveTo(x, 0);
            overlayCtx.lineTo(x, h);
            overlayCtx.stroke();
        }
        for (let y = startY; y < h; y += gridSize) {
            overlayCtx.beginPath();
            overlayCtx.moveTo(0, y);
            overlayCtx.lineTo(w, y);
            overlayCtx.stroke();
        }
    }
    
    // Draw origin axes
    const origin = worldToScreen(0, 0);
    overlayCtx.strokeStyle = '#f8514966';
    overlayCtx.lineWidth = 2;
    overlayCtx.beginPath();
    overlayCtx.moveTo(origin.x, 0);
    overlayCtx.lineTo(origin.x, h);
    overlayCtx.stroke();
    
    overlayCtx.strokeStyle = '#3fb95066';
    overlayCtx.beginPath();
    overlayCtx.moveTo(0, origin.y);
    overlayCtx.lineTo(w, origin.y);
    overlayCtx.stroke();
    
    // Draw occupancy map
    if (document.getElementById('showMap').checked && mapData && mapData.cells) {
        const cellMm = mapData.resolution;
        const cellPx = cellMm * viewScale;
        
        if (cellPx >= 0.5) {
            for (let y = 0; y < mapData.height; y++) {
                for (let x = 0; x < mapData.width; x++) {
                    const cell = mapData.cells[y * mapData.width + x];
                    if (cell > 115 && cell < 141) continue;
                    
                    const wx = mapData.origin_x + (x + 0.5) * cellMm;
                    const wy = mapData.origin_y + (y + 0.5) * cellMm;
                    const sp = worldToScreen(wx, wy);
                    
                    if (cell < 128) {
                        const c = 1.0 - cell / 128.0;
                        mapCtx.fillStyle = `rgb(${Math.floor(80+175*c)}, ${Math.floor(40*c)}, ${Math.floor(50*c)})`;
                    } else {
                        const c = (cell - 128) / 127.0;
                        const v = Math.floor(25 + 35*c);
                        mapCtx.fillStyle = `rgb(${v}, ${Math.floor(v*1.2)}, ${v})`;
                    }
                    
                    mapCtx.fillRect(sp.x - cellPx/2, sp.y - cellPx/2, Math.max(1, cellPx), Math.max(1, cellPx));
                }
            }
        }
    }
    
    // Draw trajectory
    if (document.getElementById('showTrajectory').checked && trajectory.length > 1) {
        overlayCtx.strokeStyle = '#58a6ff';
        overlayCtx.lineWidth = 2;
        overlayCtx.lineCap = 'round';
        overlayCtx.lineJoin = 'round';
        overlayCtx.beginPath();
        const first = worldToScreen(trajectory[0].x, trajectory[0].y);
        overlayCtx.moveTo(first.x, first.y);
        for (let i = 1; i < trajectory.length; i++) {
            const p = worldToScreen(trajectory[i].x, trajectory[i].y);
            overlayCtx.lineTo(p.x, p.y);
        }
        overlayCtx.stroke();
    }
    
    // Draw current scan
    if (document.getElementById('showScan').checked && currentScan.length > 0) {
        overlayCtx.fillStyle = '#f0883e';
        for (const pt of currentScan) {
            const sp = worldToScreen(pt.x, pt.y);
            overlayCtx.beginPath();
            overlayCtx.arc(sp.x, sp.y, 2, 0, Math.PI * 2);
            overlayCtx.fill();
        }
    }
    
    // Draw robot
    const rp = worldToScreen(robotPose.x, robotPose.y);
    overlayCtx.save();
    overlayCtx.translate(rp.x, rp.y);
    overlayCtx.rotate(-robotPose.theta);
    
    // Hexagon body
    const size = 18;
    overlayCtx.fillStyle = '#3fb950';
    overlayCtx.strokeStyle = '#238636';
    overlayCtx.lineWidth = 2;
    overlayCtx.beginPath();
    for (let i = 0; i < 6; i++) {
        const a = i * Math.PI / 3 - Math.PI / 6;
        const x = size * Math.cos(a);
        const y = size * Math.sin(a);
        if (i === 0) overlayCtx.moveTo(x, y);
        else overlayCtx.lineTo(x, y);
    }
    overlayCtx.closePath();
    overlayCtx.fill();
    overlayCtx.stroke();
    
    // Direction arrow
    overlayCtx.fillStyle = '#0d1117';
    overlayCtx.beginPath();
    overlayCtx.moveTo(size + 5, 0);
    overlayCtx.lineTo(0, -6);
    overlayCtx.lineTo(0, 6);
    overlayCtx.closePath();
    overlayCtx.fill();
    
    overlayCtx.restore();
    
    // Auto-center
    if (document.getElementById('autoCenter').checked) {
        viewOffset.x = w/2 - robotPose.x * viewScale;
        viewOffset.y = h/2 + robotPose.y * viewScale;
    }
    
    // Render minimap
    renderMinimap();
}

function renderMinimap() {
    const w = minimap.width;
    const h = minimap.height;
    
    minimapCtx.fillStyle = '#0d1117';
    minimapCtx.fillRect(0, 0, w, h);
    
    if (!mapData) return;
    
    // Calculate scale to fit map
    const mapWidthMm = mapData.width * mapData.resolution;
    const mapHeightMm = mapData.height * mapData.resolution;
    const scale = Math.min(w / mapWidthMm, h / mapHeightMm) * 0.9;
    const offsetX = w/2;
    const offsetY = h/2;
    
    // Draw simplified map
    if (mapData.cells) {
        const step = Math.max(1, Math.floor(4 / (mapData.resolution * scale)));
        for (let y = 0; y < mapData.height; y += step) {
            for (let x = 0; x < mapData.width; x += step) {
                const cell = mapData.cells[y * mapData.width + x];
                if (cell > 100 && cell < 156) continue;
                
                const wx = mapData.origin_x + x * mapData.resolution;
                const wy = mapData.origin_y + y * mapData.resolution;
                const sx = offsetX + wx * scale;
                const sy = offsetY - wy * scale;
                
                minimapCtx.fillStyle = cell < 128 ? '#f85149' : '#238636';
                minimapCtx.fillRect(sx, sy, Math.max(1, step * mapData.resolution * scale), Math.max(1, step * mapData.resolution * scale));
            }
        }
    }
    
    // Draw trajectory
    if (trajectory.length > 1) {
        minimapCtx.strokeStyle = '#58a6ff';
        minimapCtx.lineWidth = 1;
        minimapCtx.beginPath();
        minimapCtx.moveTo(offsetX + trajectory[0].x * scale, offsetY - trajectory[0].y * scale);
        for (const p of trajectory) {
            minimapCtx.lineTo(offsetX + p.x * scale, offsetY - p.y * scale);
        }
        minimapCtx.stroke();
    }
    
    // Draw robot
    const rx = offsetX + robotPose.x * scale;
    const ry = offsetY - robotPose.y * scale;
    minimapCtx.fillStyle = '#3fb950';
    minimapCtx.beginPath();
    minimapCtx.arc(rx, ry, 4, 0, Math.PI * 2);
    minimapCtx.fill();
    
    // Draw view rectangle
    const viewW = overlayCanvas.width / viewScale * scale;
    const viewH = overlayCanvas.height / viewScale * scale;
    const viewX = offsetX - (viewOffset.x - overlayCanvas.width/2) / viewScale * scale - viewW/2;
    const viewY = offsetY + (viewOffset.y - overlayCanvas.height/2) / viewScale * scale - viewH/2;
    
    minimapCtx.strokeStyle = '#8b949e';
    minimapCtx.lineWidth = 1;
    minimapCtx.strokeRect(viewX, viewY, viewW, viewH);
}

function resetView() {
    viewOffset = { x: overlayCanvas.width/2, y: overlayCanvas.height/2 };
    viewScale = 0.1;
    document.getElementById('zoom-level').textContent = '100%';
    render();
}

function centerOnRobot() {
    viewOffset.x = overlayCanvas.width/2 - robotPose.x * viewScale;
    viewOffset.y = overlayCanvas.height/2 + robotPose.y * viewScale;
    render();
}

function zoomIn() {
    viewScale = Math.min(0.5, viewScale * 1.3);
    document.getElementById('zoom-level').textContent = Math.round(viewScale * 1000) + '%';
    render();
}

function zoomOut() {
    viewScale = Math.max(0.02, viewScale / 1.3);
    document.getElementById('zoom-level').textContent = Math.round(viewScale * 1000) + '%';
    render();
}

// WebSocket
let ws = null;
function connect() {
    ws = new WebSocket(`ws://${location.host}/ws`);
    
    ws.onopen = () => {
        document.getElementById('status-bar').className = 'connected';
        document.getElementById('status-text').textContent = 'Connected';
    };
    
    ws.onclose = () => {
        document.getElementById('status-bar').className = 'disconnected';
        document.getElementById('status-text').textContent = 'Reconnecting...';
        setTimeout(connect, 2000);
    };
    
    ws.onerror = () => ws.close();
    
    ws.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            updateCount++;
            document.getElementById('update-count').textContent = updateCount;
            
            frameCount++;
            const now = Date.now();
            if (now - lastFrameTime > 1000) {
                document.getElementById('fps').textContent = frameCount;
                frameCount = 0;
                lastFrameTime = now;
            }
            
            if (data.pose) {
                robotPose = data.pose;
                document.getElementById('pose-x').textContent = (robotPose.x / 1000).toFixed(2);
                document.getElementById('pose-y').textContent = (robotPose.y / 1000).toFixed(2);
                document.getElementById('pose-theta').textContent = (robotPose.theta * 180 / Math.PI).toFixed(1);
            }
            
            if (data.trajectory) trajectory = data.trajectory;
            
            if (data.scan) {
                currentScan = data.scan;
                document.getElementById('point-count').textContent = currentScan.length;
            }
            
            if (data.stats) {
                document.getElementById('scan-count').textContent = data.stats.scan_count || 0;
                if (data.stats.match_quality !== undefined) {
                    matchQuality = data.stats.match_quality;
                    document.getElementById('quality').textContent = (matchQuality * 100).toFixed(0) + '%';
                    document.getElementById('quality-bar').style.width = (matchQuality * 100) + '%';
                }
            }
            
            if (data.map) {
                mapData = data.map;
                let known = 0;
                for (const c of mapData.cells) if (c < 115 || c > 141) known++;
                document.getElementById('map-cells').textContent = known.toLocaleString();
            }
            
            render();
        } catch (e) {
            console.error('Parse error:', e);
        }
    };
}

connect();
document.getElementById('zoom-level').textContent = Math.round(viewScale * 1000) + '%';
</script>
</body>
</html>
"##;

