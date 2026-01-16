// LiDAR SLAM Web Visualization Server
//
// Streams pose, scan, and map data from the LD19 through the SLAM pipeline and
// renders a live occupancy grid plus the raw measurement points.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use devices::lidar::{LidarSlamConfig, LidarSlamHandle, SlamSnapshot};
use lidar_slam::Pose2D;
use serde::Serialize;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::broadcast;

#[derive(Clone)]
struct AppState {
    slam: Arc<LidarSlamHandle>,
    tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║        LD19 LiDAR SLAM Web Visualization Server             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let config = LidarSlamConfig::default();
    println!("🔌 Connecting to LiDAR on port: {}", config.port);
    let slam = Arc::new(LidarSlamHandle::new(config)?);
    println!("✓ SLAM pipeline is running");

    let (tx, _rx) = broadcast::channel::<String>(128);
    spawn_broadcast_loop(Arc::clone(&slam), tx.clone());

    let app_state = AppState {
        slam: Arc::clone(&slam),
        tx: tx.clone(),
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(websocket_handler))
        .route("/map", get(map_handler))
        .with_state(app_state);

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

fn spawn_broadcast_loop(handle: Arc<LidarSlamHandle>, tx: broadcast::Sender<String>) {
    thread::spawn(move || {
        let mut last_frame = 0;
        loop {
            let snapshot = handle.latest();
            if snapshot.frame == 0 || snapshot.frame == last_frame {
                thread::sleep(Duration::from_millis(25));
                continue;
            }
            last_frame = snapshot.frame;
            if let Some(frame) = SlamFrame::from_snapshot(&snapshot) {
                if let Ok(payload) = serde_json::to_string(&frame) {
                    let _ = tx.send(payload);
                }
            }
        }
    });
}

#[derive(Serialize)]
struct SlamFrame {
    frame: u64,
    timestamp_ns: u64,
    pose: PoseDto,
    rpm: f32,
    points: Vec<ScanPointDto>,
}

impl SlamFrame {
    fn from_snapshot(snapshot: &SlamSnapshot) -> Option<Self> {
        let scan = snapshot.last_scan.as_ref()?;
        let rpm = if scan.rpm.is_finite() { scan.rpm } else { 0.0 };
        let points = scan
            .points
            .iter()
            .map(|p| ScanPointDto {
                angle_deg: p.angle_deg,
                distance_mm: (p.distance_m.max(0.0) * 1000.0) as u32,
                intensity: p.intensity,
            })
            .collect();

        Some(Self {
            frame: snapshot.frame,
            timestamp_ns: snapshot.timestamp_ns,
            pose: PoseDto::from_pose(snapshot.pose),
            rpm,
            points,
        })
    }
}

#[derive(Serialize)]
struct PoseDto {
    x: f32,
    y: f32,
    theta: f32,
}

impl PoseDto {
    fn from_pose(p: Pose2D) -> Self {
        Self {
            x: p.x,
            y: p.y,
            theta: p.theta,
        }
    }
}

#[derive(Serialize)]
struct ScanPointDto {
    angle_deg: f32,
    distance_mm: u32,
    intensity: u16,
}

#[derive(Serialize)]
struct MapResponse {
    frame: u64,
    width: usize,
    height: usize,
    resolution: f32,
    origin: PoseDto,
    pose: PoseDto,
    cells: Vec<i8>,
}

impl MapResponse {
    fn from_snapshot(snapshot: &SlamSnapshot) -> Option<Self> {
        let map = snapshot.map.as_ref()?;
        let origin = map.origin();
        Some(Self {
            frame: snapshot.frame,
            width: map.width(),
            height: map.height(),
            resolution: map.resolution(),
            origin: PoseDto::from_pose(origin),
            pose: PoseDto::from_pose(snapshot.pose),
            cells: map.cells().to_vec(),
        })
    }
}

async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket_connection(socket, state.tx.clone()))
}

async fn websocket_connection(mut socket: WebSocket, tx: broadcast::Sender<String>) {
    let mut rx = tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}

async fn map_handler(State(state): State<AppState>) -> impl IntoResponse {
    let snapshot = state.slam.latest();
    if let Some(map) = MapResponse::from_snapshot(&snapshot) {
        Json(map).into_response()
    } else {
        StatusCode::NO_CONTENT.into_response()
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
    <title>LD19 SLAM Viewer</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: #0c1220;
            color: #f5f5f5;
            display: flex;
            flex-direction: column;
            align-items: center;
            padding: 20px;
            gap: 20px;
        }
        h1 { color: #7dd3fc; text-shadow: 0 0 16px rgba(125, 211, 252, 0.6); }
        #stats {
            width: 100%;
            max-width: 900px;
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
            gap: 12px;
        }
        .stat {
            background: rgba(15, 23, 42, 0.9);
            padding: 14px 18px;
            border-radius: 12px;
            border: 1px solid rgba(125, 211, 252, 0.3);
            display: flex;
            flex-direction: column;
            gap: 6px;
        }
        .stat-label { font-size: 12px; letter-spacing: 0.08em; color: #9ca3af; }
        .stat-value { font-size: 24px; font-weight: 600; color: #e0f2fe; }
        #canvas-container {
            position: relative;
            background: radial-gradient(circle at center, #1e293b, #0f172a);
            border-radius: 16px;
            padding: 16px;
            box-shadow: 0 20px 60px rgba(15, 23, 42, 0.8);
        }
        canvas {
            display: block;
            border-radius: 12px;
            border: 1px solid rgba(125, 211, 252, 0.4);
        }
        #status {
            position: absolute;
            top: 24px;
            right: 24px;
            padding: 6px 14px;
            border-radius: 999px;
            background: rgba(252, 165, 3, 0.18);
            border: 1px solid rgba(252, 165, 3, 0.4);
            font-size: 12px;
            letter-spacing: 0.05em;
        }
        .connected { background: rgba(34, 197, 94, 0.2) !important; border-color: rgba(34, 197, 94, 0.5) !important; }
        .disconnected { background: rgba(248, 113, 113, 0.2) !important; border-color: rgba(248, 113, 113, 0.5) !important; }
    </style>
</head>
<body>
    <h1>LD19 SLAM Live Map</h1>
    <div id="stats">
        <div class="stat"><span class="stat-label">FRAME</span><span class="stat-value" id="frame">0</span></div>
        <div class="stat"><span class="stat-label">RPM</span><span class="stat-value" id="rpm">0.0</span></div>
        <div class="stat"><span class="stat-label">POINTS</span><span class="stat-value" id="points">0</span></div>
        <div class="stat"><span class="stat-label">POSE X / Y (m)</span><span class="stat-value" id="pose-xy">0.00 / 0.00</span></div>
        <div class="stat"><span class="stat-label">POSE θ (deg)</span><span class="stat-value" id="pose-theta">0</span></div>
        <div class="stat"><span class="stat-label">FPS</span><span class="stat-value" id="fps">0.0</span></div>
    </div>
    <div id="canvas-container">
        <canvas id="lidar-canvas" width="900" height="900"></canvas>
        <div id="status" class="disconnected">CONNECTING</div>
    </div>
    <script>
        const canvas = document.getElementById('lidar-canvas');
        const ctx = canvas.getContext('2d');
        let ws = null;
        let lastFrameTime = performance.now();
        let mapData = null;
        const mapCanvas = document.createElement('canvas');
        const mapCtx = mapCanvas.getContext('2d');

        async function fetchMap() {
            try {
                const response = await fetch('/map');
                if (!response.ok) return;
                mapData = await response.json();
                paintOccupancyMap();
            } catch (err) {
                console.warn('Map fetch failed', err);
            }
        }

        function paintOccupancyMap() {
            if (!mapData) return;
            mapCanvas.width = mapData.width;
            mapCanvas.height = mapData.height;
            const image = mapCtx.createImageData(mapData.width, mapData.height);
            for (let i = 0; i < mapData.cells.length; i++) {
                const val = mapData.cells[i];
                const norm = (val + 100) / 200;
                const shade = Math.floor(norm * 255);
                image.data[i * 4 + 0] = 20 + shade;
                image.data[i * 4 + 1] = 30 + shade;
                image.data[i * 4 + 2] = 40 + shade;
                image.data[i * 4 + 3] = 255;
            }
            mapCtx.putImageData(image, 0, 0);
        }

        function worldToCanvas(x, y) {
            if (!mapData) return null;
            const gridX = (x - mapData.origin.x) / mapData.resolution;
            const gridY = (y - mapData.origin.y) / mapData.resolution;
            const canvasX = (gridX / mapData.width) * canvas.width;
            const canvasY = (gridY / mapData.height) * canvas.height;
            return { x: canvasX, y: canvasY };
        }

        function drawPose(pose) {
            const coord = worldToCanvas(pose.x, pose.y);
            if (!coord) return;
            ctx.fillStyle = '#38bdf8';
            ctx.strokeStyle = '#e0f2fe';
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.arc(coord.x, coord.y, 6, 0, Math.PI * 2);
            ctx.fill();
            const heading = worldToCanvas(
                pose.x + Math.cos(pose.theta) * 0.3,
                pose.y + Math.sin(pose.theta) * 0.3
            );
            if (heading) {
                ctx.beginPath();
                ctx.moveTo(coord.x, coord.y);
                ctx.lineTo(heading.x, heading.y);
                ctx.stroke();
            }
        }

        function drawScan(pose, points) {
            if (!mapData) return;
            ctx.fillStyle = 'rgba(248, 113, 113, 0.8)';
            points.forEach(point => {
                const rangeM = point.distance_mm / 1000;
                const worldX = pose.x + rangeM * Math.cos(point.angle_deg * Math.PI / 180 + pose.theta);
                const worldY = pose.y + rangeM * Math.sin(point.angle_deg * Math.PI / 180 + pose.theta);
                const coord = worldToCanvas(worldX, worldY);
                if (!coord) return;
                ctx.beginPath();
                ctx.arc(coord.x, coord.y, 2, 0, Math.PI * 2);
                ctx.fill();
            });
        }

        function drawFrame(data) {
            ctx.fillStyle = '#020617';
            ctx.fillRect(0, 0, canvas.width, canvas.height);
            if (mapData) {
                ctx.drawImage(mapCanvas, 0, 0, canvas.width, canvas.height);
            }
            drawPose(data.pose);
            drawScan(data.pose, data.points);
        }

        function updateStats(data) {
            document.getElementById('frame').textContent = data.frame;
            document.getElementById('rpm').textContent = data.rpm.toFixed(1);
            document.getElementById('points').textContent = data.points.length;
            document.getElementById('pose-xy').textContent = `${data.pose.x.toFixed(2)} / ${data.pose.y.toFixed(2)}`;
            document.getElementById('pose-theta').textContent = (data.pose.theta * 180 / Math.PI).toFixed(1);
            const now = performance.now();
            const fps = 1000 / (now - lastFrameTime);
            document.getElementById('fps').textContent = fps.toFixed(1);
            lastFrameTime = now;
        }

        function connectWebSocket() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
            ws.onopen = () => {
                const status = document.getElementById('status');
                status.textContent = 'CONNECTED';
                status.className = 'connected';
            };
            ws.onclose = () => {
                const status = document.getElementById('status');
                status.textContent = 'RECONNECTING...';
                status.className = 'disconnected';
                setTimeout(connectWebSocket, 2000);
            };
            ws.onmessage = (event) => {
                const data = JSON.parse(event.data);
                drawFrame(data);
                updateStats(data);
            };
        }

        fetchMap();
        setInterval(fetchMap, 2500);
        connectWebSocket();
    </script>
</body>
</html>"#;
