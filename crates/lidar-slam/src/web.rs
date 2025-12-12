//! Web visualization server for SLAM
//!
//! Provides a web interface to visualize the occupancy grid map,
//! robot trajectory, and real-time LiDAR scans.

use crate::slam::SlamProcessor;
use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{Html, IntoResponse},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use tower_http::cors::{Any, CorsLayer};

/// Shared SLAM state for web server
pub type SharedSlam = Arc<Mutex<SlamProcessor>>;

/// Web server for SLAM visualization
pub struct SlamWebServer {
    slam: SharedSlam,
    tx: broadcast::Sender<String>,
}

/// Message sent to web clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlamUpdate {
    /// Current robot pose
    pub pose: PoseData,
    /// Latest scan points (in world coordinates)
    pub scan: Vec<PointData>,
    /// Map update (if changed)
    pub map: Option<MapData>,
    /// Trajectory
    pub trajectory: Vec<PoseData>,
    /// Statistics
    pub stats: StatsData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseData {
    pub x: f32,
    pub y: f32,
    pub theta: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapData {
    pub width: usize,
    pub height: usize,
    pub resolution: f32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub cells: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsData {
    pub scan_count: u64,
    pub match_quality: f32,
    pub is_initialized: bool,
    pub map_updates: u64,
}

impl SlamWebServer {
    /// Create a new web server instance
    pub fn new(slam: SharedSlam) -> Self {
        let (tx, _) = broadcast::channel::<String>(100);
        Self { slam, tx }
    }

    /// Get the broadcast sender for publishing updates
    pub fn get_sender(&self) -> broadcast::Sender<String> {
        self.tx.clone()
    }

    /// Build the Axum router
    pub fn build_router(self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            .route("/", get(index_handler))
            .route("/ws", get(websocket_handler))
            .route("/api/state", get(state_handler))
            .route("/api/map", get(map_handler))
            .route("/api/reset", get(reset_handler))
            .layer(cors)
            .with_state((self.slam, self.tx))
    }

    /// Broadcast an update to all connected clients
    pub async fn broadcast_update(&self, scan_points: &[crate::types::Point2D]) {
        let slam = self.slam.lock().await;

        let update = SlamUpdate {
            pose: PoseData {
                x: slam.current_pose().x,
                y: slam.current_pose().y,
                theta: slam.current_pose().theta,
            },
            scan: scan_points
                .iter()
                .map(|p| PointData { x: p.x, y: p.y })
                .collect(),
            map: Some(MapData {
                width: slam.get_map().dimensions().0,
                height: slam.get_map().dimensions().1,
                resolution: slam.get_map().resolution(),
                origin_x: slam.get_map().origin().x,
                origin_y: slam.get_map().origin().y,
                cells: slam.get_map().to_image_data(),
            }),
            trajectory: slam
                .trajectory()
                .iter()
                .map(|p| PoseData {
                    x: p.x,
                    y: p.y,
                    theta: p.theta,
                })
                .collect(),
            stats: StatsData {
                scan_count: slam.scan_count(),
                match_quality: slam.get_state().match_quality,
                is_initialized: slam.is_initialized(),
                map_updates: slam.get_map().update_count(),
            },
        };

        if let Ok(json) = serde_json::to_string(&update) {
            let _ = self.tx.send(json);
        }
    }
}

type AppState = (SharedSlam, broadcast::Sender<String>);

async fn index_handler() -> Html<&'static str> {
    Html(SLAM_HTML)
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State((slam, tx)): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, slam, tx))
}

async fn handle_websocket(mut socket: WebSocket, slam: SharedSlam, tx: broadcast::Sender<String>) {
    let mut rx = tx.subscribe();

    // Send initial state
    {
        let slam = slam.lock().await;
        let state = slam.to_json();
        if let Ok(json) = serde_json::to_string(&state) {
            let _ = socket.send(Message::Text(json.into())).await;
        }
    }

    // Forward updates to client
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(data) => {
                        if socket.send(Message::Text(data.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

async fn state_handler(State((slam, _)): State<AppState>) -> impl IntoResponse {
    let slam = slam.lock().await;
    axum::Json(slam.get_state())
}

async fn map_handler(State((slam, _)): State<AppState>) -> impl IntoResponse {
    let slam = slam.lock().await;
    axum::Json(slam.get_map().to_json())
}

async fn reset_handler(State((slam, _)): State<AppState>) -> impl IntoResponse {
    let mut slam = slam.lock().await;
    slam.reset();
    axum::Json(serde_json::json!({"status": "reset"}))
}

/// Embedded HTML for the SLAM visualization
const SLAM_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Hexapod SLAM Visualization</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        body {
            font-family: 'Segoe UI', system-ui, sans-serif;
            background: #1a1a2e;
            color: #eee;
            height: 100vh;
            display: flex;
            flex-direction: column;
        }
        header {
            background: #16213e;
            padding: 1rem 2rem;
            display: flex;
            justify-content: space-between;
            align-items: center;
            border-bottom: 2px solid #0f3460;
        }
        h1 {
            color: #00ff88;
            font-size: 1.5rem;
        }
        .status {
            display: flex;
            gap: 2rem;
        }
        .status-item {
            text-align: center;
        }
        .status-label {
            font-size: 0.75rem;
            color: #888;
            text-transform: uppercase;
        }
        .status-value {
            font-size: 1.2rem;
            font-weight: bold;
            color: #00ff88;
        }
        main {
            flex: 1;
            display: flex;
            padding: 1rem;
            gap: 1rem;
        }
        .map-container {
            flex: 1;
            background: #0f0f23;
            border-radius: 8px;
            overflow: hidden;
            position: relative;
        }
        #mapCanvas {
            width: 100%;
            height: 100%;
            display: block;
        }
        .sidebar {
            width: 300px;
            display: flex;
            flex-direction: column;
            gap: 1rem;
        }
        .panel {
            background: #16213e;
            border-radius: 8px;
            padding: 1rem;
        }
        .panel h3 {
            color: #00ff88;
            margin-bottom: 0.5rem;
            font-size: 0.9rem;
            text-transform: uppercase;
        }
        .pose-grid {
            display: grid;
            grid-template-columns: 1fr 1fr 1fr;
            gap: 0.5rem;
        }
        .pose-item {
            text-align: center;
            padding: 0.5rem;
            background: #0f0f23;
            border-radius: 4px;
        }
        .pose-label {
            font-size: 0.7rem;
            color: #888;
        }
        .pose-value {
            font-size: 1rem;
            font-weight: bold;
        }
        .controls {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
        }
        button {
            padding: 0.75rem;
            background: #0f3460;
            border: none;
            border-radius: 4px;
            color: #fff;
            cursor: pointer;
            font-size: 0.9rem;
            transition: background 0.2s;
        }
        button:hover {
            background: #1a5490;
        }
        button.danger {
            background: #e94560;
        }
        button.danger:hover {
            background: #ff6b6b;
        }
        .legend {
            display: flex;
            flex-wrap: wrap;
            gap: 0.5rem;
            font-size: 0.8rem;
        }
        .legend-item {
            display: flex;
            align-items: center;
            gap: 0.25rem;
        }
        .legend-color {
            width: 16px;
            height: 16px;
            border-radius: 2px;
        }
        .connection-status {
            position: fixed;
            bottom: 1rem;
            right: 1rem;
            padding: 0.5rem 1rem;
            border-radius: 20px;
            font-size: 0.8rem;
        }
        .connected { background: #00ff88; color: #000; }
        .disconnected { background: #e94560; color: #fff; }
        #lidarCanvas {
            width: 100%;
            height: 200px;
            background: #0f0f23;
            border-radius: 4px;
        }
    </style>
</head>
<body>
    <header>
        <h1>🦾 Hexapod SLAM</h1>
        <div class="status">
            <div class="status-item">
                <div class="status-label">Scans</div>
                <div class="status-value" id="scanCount">0</div>
            </div>
            <div class="status-item">
                <div class="status-label">Match Quality</div>
                <div class="status-value" id="matchQuality">0%</div>
            </div>
            <div class="status-item">
                <div class="status-label">Map Updates</div>
                <div class="status-value" id="mapUpdates">0</div>
            </div>
        </div>
    </header>
    
    <main>
        <div class="map-container">
            <canvas id="mapCanvas"></canvas>
        </div>
        
        <div class="sidebar">
            <div class="panel">
                <h3>Robot Pose</h3>
                <div class="pose-grid">
                    <div class="pose-item">
                        <div class="pose-label">X (mm)</div>
                        <div class="pose-value" id="poseX">0</div>
                    </div>
                    <div class="pose-item">
                        <div class="pose-label">Y (mm)</div>
                        <div class="pose-value" id="poseY">0</div>
                    </div>
                    <div class="pose-item">
                        <div class="pose-label">θ (deg)</div>
                        <div class="pose-value" id="poseTheta">0</div>
                    </div>
                </div>
            </div>
            
            <div class="panel">
                <h3>Current Scan</h3>
                <canvas id="lidarCanvas"></canvas>
            </div>
            
            <div class="panel">
                <h3>Legend</h3>
                <div class="legend">
                    <div class="legend-item">
                        <div class="legend-color" style="background: #808080;"></div>
                        <span>Unknown</span>
                    </div>
                    <div class="legend-item">
                        <div class="legend-color" style="background: #ffffff;"></div>
                        <span>Free</span>
                    </div>
                    <div class="legend-item">
                        <div class="legend-color" style="background: #000000;"></div>
                        <span>Occupied</span>
                    </div>
                    <div class="legend-item">
                        <div class="legend-color" style="background: #00ff88;"></div>
                        <span>Robot</span>
                    </div>
                    <div class="legend-item">
                        <div class="legend-color" style="background: #ff6b6b;"></div>
                        <span>Scan</span>
                    </div>
                    <div class="legend-item">
                        <div class="legend-color" style="background: #4ecdc4;"></div>
                        <span>Path</span>
                    </div>
                </div>
            </div>
            
            <div class="panel controls">
                <h3>Controls</h3>
                <button onclick="resetSlam()" class="danger">Reset SLAM</button>
                <button onclick="toggleAutoCenter()">Toggle Auto-Center</button>
                <button onclick="zoomIn()">Zoom In</button>
                <button onclick="zoomOut()">Zoom Out</button>
            </div>
        </div>
    </main>
    
    <div class="connection-status disconnected" id="connectionStatus">Disconnected</div>
    
    <script>
        const mapCanvas = document.getElementById('mapCanvas');
        const mapCtx = mapCanvas.getContext('2d');
        const lidarCanvas = document.getElementById('lidarCanvas');
        const lidarCtx = lidarCanvas.getContext('2d');
        
        let ws = null;
        let slamData = null;
        let autoCenter = true;
        let zoom = 1;
        let panX = 0, panY = 0;
        let mapImage = null;
        
        function resizeCanvases() {
            const container = mapCanvas.parentElement;
            mapCanvas.width = container.clientWidth;
            mapCanvas.height = container.clientHeight;
            lidarCanvas.width = lidarCanvas.clientWidth;
            lidarCanvas.height = lidarCanvas.clientHeight;
            render();
        }
        
        window.addEventListener('resize', resizeCanvases);
        resizeCanvases();
        
        function connect() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
            
            ws.onopen = () => {
                document.getElementById('connectionStatus').className = 'connection-status connected';
                document.getElementById('connectionStatus').textContent = 'Connected';
            };
            
            ws.onclose = () => {
                document.getElementById('connectionStatus').className = 'connection-status disconnected';
                document.getElementById('connectionStatus').textContent = 'Disconnected';
                setTimeout(connect, 2000);
            };
            
            ws.onerror = () => {
                ws.close();
            };
            
            ws.onmessage = (event) => {
                try {
                    slamData = JSON.parse(event.data);
                    updateUI();
                    render();
                } catch (e) {
                    console.error('Failed to parse SLAM data:', e);
                }
            };
        }
        
        function updateUI() {
            if (!slamData) return;
            
            document.getElementById('scanCount').textContent = slamData.stats?.scan_count || 0;
            document.getElementById('matchQuality').textContent = 
                ((slamData.stats?.match_quality || 0) * 100).toFixed(0) + '%';
            document.getElementById('mapUpdates').textContent = slamData.stats?.map_updates || 0;
            
            const pose = slamData.pose;
            if (pose) {
                document.getElementById('poseX').textContent = pose.x.toFixed(0);
                document.getElementById('poseY').textContent = pose.y.toFixed(0);
                document.getElementById('poseTheta').textContent = (pose.theta * 180 / Math.PI).toFixed(1);
            }
        }
        
        function render() {
            if (!slamData) return;
            
            // Clear canvas
            mapCtx.fillStyle = '#0f0f23';
            mapCtx.fillRect(0, 0, mapCanvas.width, mapCanvas.height);
            
            // Calculate transform
            const map = slamData.map;
            if (!map) return;
            
            const scale = zoom * Math.min(
                mapCanvas.width / (map.width * map.resolution),
                mapCanvas.height / (map.height * map.resolution)
            ) * 0.9;
            
            mapCtx.save();
            mapCtx.translate(mapCanvas.width / 2, mapCanvas.height / 2);
            
            if (autoCenter && slamData.pose) {
                panX = -slamData.pose.x * scale / 1000;
                panY = slamData.pose.y * scale / 1000;
            }
            mapCtx.translate(panX, panY);
            mapCtx.scale(scale, -scale); // Flip Y for standard coordinate system
            
            // Draw map
            if (map.cells) {
                const imageData = mapCtx.createImageData(map.width, map.height);
                for (let i = 0; i < map.cells.length; i++) {
                    const v = map.cells[i];
                    const idx = i * 4;
                    if (v === 128) {
                        // Unknown - dark gray
                        imageData.data[idx] = 64;
                        imageData.data[idx + 1] = 64;
                        imageData.data[idx + 2] = 64;
                    } else {
                        // 0=occupied (black), 255=free (white)
                        imageData.data[idx] = v;
                        imageData.data[idx + 1] = v;
                        imageData.data[idx + 2] = v;
                    }
                    imageData.data[idx + 3] = 255;
                }
                
                // Create temporary canvas for the map image
                const tempCanvas = document.createElement('canvas');
                tempCanvas.width = map.width;
                tempCanvas.height = map.height;
                const tempCtx = tempCanvas.getContext('2d');
                tempCtx.putImageData(imageData, 0, 0);
                
                // Draw map image
                mapCtx.save();
                mapCtx.translate(map.origin_x / 1000, map.origin_y / 1000);
                mapCtx.scale(map.resolution / 1000, map.resolution / 1000);
                mapCtx.scale(1, -1); // Flip back for image
                mapCtx.translate(0, -map.height);
                mapCtx.drawImage(tempCanvas, 0, 0);
                mapCtx.restore();
            }
            
            // Draw trajectory
            if (slamData.trajectory && slamData.trajectory.length > 1) {
                mapCtx.beginPath();
                mapCtx.strokeStyle = '#4ecdc4';
                mapCtx.lineWidth = 2 / scale;
                slamData.trajectory.forEach((p, i) => {
                    const x = p.x / 1000;
                    const y = p.y / 1000;
                    if (i === 0) mapCtx.moveTo(x, y);
                    else mapCtx.lineTo(x, y);
                });
                mapCtx.stroke();
            }
            
            // Draw current scan points
            if (slamData.scan) {
                mapCtx.fillStyle = '#ff6b6b';
                slamData.scan.forEach(p => {
                    mapCtx.beginPath();
                    mapCtx.arc(p.x / 1000, p.y / 1000, 3 / scale, 0, Math.PI * 2);
                    mapCtx.fill();
                });
            }
            
            // Draw robot
            if (slamData.pose) {
                const p = slamData.pose;
                const x = p.x / 1000;
                const y = p.y / 1000;
                const size = 30 / scale;
                
                mapCtx.save();
                mapCtx.translate(x, y);
                mapCtx.rotate(p.theta);
                
                // Robot body (hexagon-ish shape for hexapod)
                mapCtx.fillStyle = '#00ff88';
                mapCtx.beginPath();
                for (let i = 0; i < 6; i++) {
                    const angle = (i * Math.PI / 3) - Math.PI / 6;
                    const rx = size * Math.cos(angle);
                    const ry = size * Math.sin(angle);
                    if (i === 0) mapCtx.moveTo(rx, ry);
                    else mapCtx.lineTo(rx, ry);
                }
                mapCtx.closePath();
                mapCtx.fill();
                
                // Direction indicator
                mapCtx.fillStyle = '#fff';
                mapCtx.beginPath();
                mapCtx.moveTo(size * 1.2, 0);
                mapCtx.lineTo(size * 0.5, size * 0.3);
                mapCtx.lineTo(size * 0.5, -size * 0.3);
                mapCtx.closePath();
                mapCtx.fill();
                
                mapCtx.restore();
            }
            
            mapCtx.restore();
            
            // Draw LiDAR visualization
            renderLidar();
        }
        
        function renderLidar() {
            lidarCtx.fillStyle = '#0f0f23';
            lidarCtx.fillRect(0, 0, lidarCanvas.width, lidarCanvas.height);
            
            if (!slamData?.scan) return;
            
            const cx = lidarCanvas.width / 2;
            const cy = lidarCanvas.height / 2;
            const maxRange = 4000; // 4m display range
            const scale = Math.min(cx, cy) * 0.9 / maxRange;
            
            // Draw range circles
            lidarCtx.strokeStyle = '#333';
            lidarCtx.lineWidth = 1;
            for (let r = 1000; r <= maxRange; r += 1000) {
                lidarCtx.beginPath();
                lidarCtx.arc(cx, cy, r * scale, 0, Math.PI * 2);
                lidarCtx.stroke();
            }
            
            // Draw points (in robot frame)
            lidarCtx.fillStyle = '#ff6b6b';
            if (slamData.pose) {
                const cos = Math.cos(-slamData.pose.theta);
                const sin = Math.sin(-slamData.pose.theta);
                slamData.scan.forEach(p => {
                    // Transform from world to robot frame
                    const dx = p.x - slamData.pose.x;
                    const dy = p.y - slamData.pose.y;
                    const rx = dx * cos - dy * sin;
                    const ry = dx * sin + dy * cos;
                    
                    const x = cx + rx * scale;
                    const y = cy - ry * scale;
                    lidarCtx.beginPath();
                    lidarCtx.arc(x, y, 2, 0, Math.PI * 2);
                    lidarCtx.fill();
                });
            }
            
            // Draw robot center
            lidarCtx.fillStyle = '#00ff88';
            lidarCtx.beginPath();
            lidarCtx.arc(cx, cy, 4, 0, Math.PI * 2);
            lidarCtx.fill();
        }
        
        function resetSlam() {
            fetch('/api/reset').then(() => console.log('SLAM reset'));
        }
        
        function toggleAutoCenter() {
            autoCenter = !autoCenter;
        }
        
        function zoomIn() {
            zoom *= 1.2;
            render();
        }
        
        function zoomOut() {
            zoom /= 1.2;
            render();
        }
        
        // Mouse/touch pan
        let isDragging = false;
        let lastX, lastY;
        
        mapCanvas.addEventListener('mousedown', (e) => {
            isDragging = true;
            lastX = e.clientX;
            lastY = e.clientY;
            autoCenter = false;
        });
        
        mapCanvas.addEventListener('mousemove', (e) => {
            if (!isDragging) return;
            panX += e.clientX - lastX;
            panY += e.clientY - lastY;
            lastX = e.clientX;
            lastY = e.clientY;
            render();
        });
        
        mapCanvas.addEventListener('mouseup', () => isDragging = false);
        mapCanvas.addEventListener('mouseleave', () => isDragging = false);
        
        mapCanvas.addEventListener('wheel', (e) => {
            e.preventDefault();
            const factor = e.deltaY > 0 ? 0.9 : 1.1;
            zoom *= factor;
            render();
        });
        
        // Start connection
        connect();
    </script>
</body>
</html>
"##;
