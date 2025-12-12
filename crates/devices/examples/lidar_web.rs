//! LiDAR Web Visualization Server
//!
//! This example creates a web server that displays real-time LiDAR data
//! in an interactive HTML canvas visualization.
//!
//! # Usage
//!
//! On Raspberry Pi with real hardware:
//! ```bash
//! cargo run --example lidar_web --features real
//! ```
//!
//! Then open in your browser: http://192.168.1.XXX:3001
//!
//! For testing on PC (dummy mode):
//! ```bash
//! cargo run --example lidar_web --features dummy
//! ```
//! Then open: http://localhost:3001

use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
};
use devices::lidar::LidarDriver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::broadcast;

type SharedDriver = Arc<Mutex<LidarDriver>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║        LD19 LiDAR Web Visualization Server                  ║");
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
            Arc::new(Mutex::new(d))
        }
        Err(e) => {
            eprintln!("✗ Failed to open serial port: {}", e);
            return Err(e);
        }
    };

    // Create broadcast channel for WebSocket updates
    let (tx, _rx) = broadcast::channel::<String>(100);

    // Spawn thread to read LiDAR and broadcast updates
    let driver_clone = Arc::clone(&driver);
    let tx_clone = tx.clone();
    thread::spawn(move || {
        lidar_broadcast_loop(driver_clone, tx_clone);
    });

    // Build the router
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(websocket_handler))
        .with_state((driver, tx));

    let addr = "0.0.0.0:3001";
    println!();
    println!("🌐 Web server starting on {}", addr);
    println!("📱 Open in browser: http://localhost:3001");
    println!("   (or http://YOUR_PI_IP:3001 from another device)");
    println!();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn lidar_broadcast_loop(driver: SharedDriver, tx: broadcast::Sender<String>) {
    let mut frame_count = 0;
    loop {
        let is_ready = {
            let driver = driver.lock().unwrap();
            driver.is_frame_ready()
        };

        if is_ready {
            let cloud = {
                let driver = driver.lock().unwrap();
                driver.get_point_cloud()
            };

            if let Some(cloud) = cloud {
                frame_count += 1;

                // Convert to JSON
                let mut points_json = String::from("[");
                for (i, point) in cloud.valid_points().enumerate() {
                    if i > 0 {
                        points_json.push(',');
                    }
                    points_json.push_str(&format!(
                        "{{\"angle\":{:.2},\"distance\":{},\"intensity\":{}}}",
                        point.angle, point.distance, point.intensity
                    ));
                }
                points_json.push(']');

                let data = format!(
                    "{{\"frame\":{},\"speed\":{:.2},\"timestamp\":{},\"points\":{}}}",
                    frame_count,
                    cloud.frequency(),
                    cloud.timestamp,
                    points_json
                );

                let _ = tx.send(data);
            }
        }

        thread::sleep(Duration::from_millis(50)); // Send updates at 20Hz max
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    axum::extract::State((_, tx)): axum::extract::State<(SharedDriver, broadcast::Sender<String>)>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, tx))
}

async fn websocket_connection(mut socket: WebSocket, tx: broadcast::Sender<String>) {
    let mut rx = tx.subscribe();

    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}

async fn index_handler() -> Html<&'static str> {
    Html(HTML_CONTENT)
}

const HTML_CONTENT: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>LD19 LiDAR Visualization</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: #1a1a2e;
            color: #eee;
            display: flex;
            flex-direction: column;
            align-items: center;
            padding: 20px;
        }
        h1 {
            margin-bottom: 10px;
            color: #00d9ff;
            text-shadow: 0 0 10px rgba(0, 217, 255, 0.5);
        }
        #stats {
            background: #16213e;
            padding: 15px 30px;
            border-radius: 10px;
            margin-bottom: 20px;
            box-shadow: 0 4px 15px rgba(0, 0, 0, 0.3);
            display: flex;
            gap: 30px;
            flex-wrap: wrap;
        }
        .stat {
            display: flex;
            flex-direction: column;
            align-items: center;
        }
        .stat-label {
            font-size: 12px;
            color: #888;
            text-transform: uppercase;
        }
        .stat-value {
            font-size: 24px;
            font-weight: bold;
            color: #00d9ff;
        }
        #canvas-container {
            position: relative;
            background: #0f1419;
            border-radius: 15px;
            padding: 20px;
            box-shadow: 0 8px 30px rgba(0, 0, 0, 0.5);
        }
        canvas {
            display: block;
            border: 2px solid #00d9ff;
            border-radius: 10px;
            box-shadow: 0 0 20px rgba(0, 217, 255, 0.3);
        }
        #status {
            position: absolute;
            top: 30px;
            right: 30px;
            padding: 10px 20px;
            background: rgba(0, 217, 255, 0.2);
            border-radius: 5px;
            font-size: 14px;
        }
        .connected { background: rgba(0, 255, 100, 0.2) !important; }
        .disconnected { background: rgba(255, 50, 50, 0.2) !important; }
    </style>
</head>
<body>
    <h1>🔄 LD19 LiDAR Live Visualization</h1>
    
    <div id="stats">
        <div class="stat">
            <span class="stat-label">Frame</span>
            <span class="stat-value" id="frame">0</span>
        </div>
        <div class="stat">
            <span class="stat-label">Speed (Hz)</span>
            <span class="stat-value" id="speed">0.0</span>
        </div>
        <div class="stat">
            <span class="stat-label">Points</span>
            <span class="stat-value" id="points">0</span>
        </div>
        <div class="stat">
            <span class="stat-label">FPS</span>
            <span class="stat-value" id="fps">0.0</span>
        </div>
    </div>

    <div id="canvas-container">
        <canvas id="lidar-canvas" width="800" height="800"></canvas>
        <div id="status" class="disconnected">Connecting...</div>
    </div>

    <script>
        const canvas = document.getElementById('lidar-canvas');
        const ctx = canvas.getContext('2d');
        const centerX = canvas.width / 2;
        const centerY = canvas.height / 2;
        const maxRange = 4000; // 4 meters in mm
        const scale = Math.min(centerX, centerY) / maxRange;

        let frameCount = 0;
        let lastFrameTime = Date.now();
        let ws = null;

        function connectWebSocket() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
            
            ws.onopen = () => {
                document.getElementById('status').textContent = 'Connected';
                document.getElementById('status').className = 'connected';
            };
            
            ws.onclose = () => {
                document.getElementById('status').textContent = 'Disconnected';
                document.getElementById('status').className = 'disconnected';
                setTimeout(connectWebSocket, 2000);
            };
            
            ws.onmessage = (event) => {
                const data = JSON.parse(event.data);
                drawLidarData(data);
                updateStats(data);
            };
        }

        function drawLidarData(data) {
            // Clear canvas with fade effect
            ctx.fillStyle = 'rgba(15, 20, 25, 0.3)';
            ctx.fillRect(0, 0, canvas.width, canvas.height);
            
            // Draw grid
            ctx.strokeStyle = 'rgba(0, 217, 255, 0.1)';
            ctx.lineWidth = 1;
            for (let i = 1; i <= 4; i++) {
                const radius = (i * 1000) * scale;
                ctx.beginPath();
                ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
                ctx.stroke();
            }
            
            // Draw cardinal directions
            ctx.strokeStyle = 'rgba(0, 217, 255, 0.3)';
            ctx.beginPath();
            ctx.moveTo(centerX, 0);
            ctx.lineTo(centerX, canvas.height);
            ctx.moveTo(0, centerY);
            ctx.lineTo(canvas.width, centerY);
            ctx.stroke();
            
            // Draw sensor
            ctx.fillStyle = '#00d9ff';
            ctx.beginPath();
            ctx.arc(centerX, centerY, 5, 0, Math.PI * 2);
            ctx.fill();
            
            // Draw points
            data.points.forEach(point => {
                const angle = (point.angle - 90) * Math.PI / 180;
                const distance = point.distance * scale;
                const x = centerX + distance * Math.cos(angle);
                const y = centerY + distance * Math.sin(angle);
                
                // Color based on intensity and distance
                const intensity = point.intensity / 255;
                const distanceFactor = Math.min(point.distance / maxRange, 1);
                const hue = 120 - distanceFactor * 60; // Green to red
                const alpha = 0.5 + intensity * 0.5;
                
                ctx.fillStyle = `hsla(${hue}, 100%, 50%, ${alpha})`;
                ctx.beginPath();
                ctx.arc(x, y, 3, 0, Math.PI * 2);
                ctx.fill();
            });
        }

        function updateStats(data) {
            document.getElementById('frame').textContent = data.frame;
            document.getElementById('speed').textContent = data.speed.toFixed(2);
            document.getElementById('points').textContent = data.points.length;
            
            const now = Date.now();
            const fps = 1000 / (now - lastFrameTime);
            document.getElementById('fps').textContent = fps.toFixed(1);
            lastFrameTime = now;
        }

        connectWebSocket();
    </script>
</body>
</html>"#;
