const DEFAULT_API_PORT = 3000;
const QUERY = new URLSearchParams(window.location.search);
const API_OVERRIDE = QUERY.get('api');

function inferApiBase() {
    if (API_OVERRIDE) {
        return API_OVERRIDE.replace(/\/$/, '');
    }
    const { protocol, hostname, port } = window.location;
    let targetPort = port;
    if (!targetPort) {
        targetPort = `${DEFAULT_API_PORT}`;
    } else if (port === '8080') {
        targetPort = `${DEFAULT_API_PORT}`;
    }
    return `${protocol}//${hostname}:${targetPort}/api`;
}

const API_BASE = inferApiBase();
document.getElementById('api-base').textContent = API_BASE;

const canvas = document.getElementById('map-canvas');
const ctx = canvas.getContext('2d');
const statusBadge = document.getElementById('status');
let lastFrameTs = performance.now();
let mapData = null;
const mapCanvas = document.createElement('canvas');
const mapCtx = mapCanvas.getContext('2d');

function setStatus(text, connected) {
    statusBadge.textContent = text;
    statusBadge.className = connected ? 'connected' : 'disconnected';
}

async function fetchFrame() {
    try {
        const res = await fetch(`${API_BASE}/lidar/frame`);
        if (!res.ok) {
            if (res.status !== 204) {
                setStatus('No data', false);
            }
            return;
        }
        const data = await res.json();
        updateStats(data);
        drawScene(data);
        setStatus('Live', true);
    } catch (err) {
        console.error('Frame fetch failed', err);
        setStatus('Disconnected', false);
    }
}

async function fetchMap() {
    try {
        const res = await fetch(`${API_BASE}/lidar/map`);
        if (!res.ok) return;
        mapData = await res.json();
        paintOccupancy();
    } catch (err) {
        console.warn('Map fetch failed', err);
    }
}

function paintOccupancy() {
    if (!mapData) return;
    mapCanvas.width = mapData.width;
    mapCanvas.height = mapData.height;
    const image = mapCtx.createImageData(mapData.width, mapData.height);
    for (let i = 0; i < mapData.cells.length; i++) {
        const v = mapData.cells[i];
        const normalized = (v + 90) / 180;
        const shade = Math.max(0, Math.min(255, Math.round(normalized * 255)));
        const base = i * 4;
        image.data[base + 0] = 18 + shade;
        image.data[base + 1] = 25 + shade;
        image.data[base + 2] = 35 + shade;
        image.data[base + 3] = 255;
    }
    mapCtx.putImageData(image, 0, 0);
}

function worldToCanvas(x, y) {
    if (!mapData) return null;
    const gridX = (x - mapData.origin.x) / mapData.resolution;
    const gridY = (y - mapData.origin.y) / mapData.resolution;
    const canvasX = (gridX / mapData.width) * canvas.width;
    const canvasY = (gridY / mapData.height) * canvas.height;
    if (canvasX < 0 || canvasY < 0 || canvasX > canvas.width || canvasY > canvas.height) {
        return null;
    }
    return { x: canvasX, y: canvasY };
}

function drawScene(frame) {
    ctx.fillStyle = '#020617';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    if (mapCanvas.width && mapCanvas.height) {
        ctx.drawImage(mapCanvas, 0, 0, canvas.width, canvas.height);
    }
    drawPose(frame.pose);
    drawScan(frame.pose, frame.points);
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
        pose.x + Math.cos(pose.theta) * 0.4,
        pose.y + Math.sin(pose.theta) * 0.4
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
    ctx.fillStyle = 'rgba(248, 113, 113, 0.85)';
    for (const point of points) {
        const rangeM = point.distance_mm / 1000;
        const worldX = pose.x + rangeM * Math.cos(point.angle_deg * Math.PI / 180 + pose.theta);
        const worldY = pose.y + rangeM * Math.sin(point.angle_deg * Math.PI / 180 + pose.theta);
        const coord = worldToCanvas(worldX, worldY);
        if (!coord) continue;
        ctx.beginPath();
        ctx.arc(coord.x, coord.y, 2, 0, Math.PI * 2);
        ctx.fill();
    }
}

function updateStats(frame) {
    document.getElementById('frame').textContent = frame.frame;
    document.getElementById('rpm').textContent = frame.rpm.toFixed(1);
    document.getElementById('points').textContent = frame.points.length;
    document.getElementById('pose-xy').textContent = `${frame.pose.x.toFixed(2)} / ${frame.pose.y.toFixed(2)}`;
    document.getElementById('pose-theta').textContent = `${(frame.pose.theta * 180 / Math.PI).toFixed(1)}°`;
    const now = performance.now();
    const fps = 1000 / (now - lastFrameTs);
    document.getElementById('fps').textContent = fps.toFixed(1);
    lastFrameTs = now;
}

setInterval(fetchFrame, 200);
setInterval(fetchMap, 2500);
fetchFrame();
fetchMap();
setStatus('Connecting…', false);
