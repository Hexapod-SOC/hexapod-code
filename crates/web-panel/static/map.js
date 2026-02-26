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
const AI_API_BASE = `${window.location.protocol}//${window.location.hostname}:3001/api/ai`;

const canvas = document.getElementById('map-canvas');
const ctx = canvas.getContext('2d');
const statusBadge = document.getElementById('status');
let lastFrameTs = performance.now();
let mapData = null;
const mapCanvas = document.createElement('canvas');
const mapCtx = mapCanvas.getContext('2d');
const navStatus = document.getElementById('nav-status');
const navList = document.getElementById('nav-list');
const navSendBtn = document.getElementById('nav-send');
const navAppendBtn = document.getElementById('nav-append');
const navClearBtn = document.getElementById('nav-clear');

let waypoints = [];

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
    drawWaypoints();
}

function drawWaypoints() {
    if (!mapData || waypoints.length === 0) return;
    waypoints.forEach((wp, idx) => {
        const coord = worldToCanvas(wp.x, wp.y);
        if (!coord) return;
        ctx.fillStyle = '#fbbf24';
        ctx.beginPath();
        ctx.arc(coord.x, coord.y, 5, 0, Math.PI * 2);
        ctx.fill();

        ctx.fillStyle = '#0f172a';
        ctx.font = '12px Segoe UI';
        ctx.fillText(`${idx + 1}`, coord.x + 7, coord.y - 7);
    });
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

function canvasToWorld(clientX, clientY) {
    if (!mapData) return null;
    const rect = canvas.getBoundingClientRect();
    const x = clientX - rect.left;
    const y = clientY - rect.top;
    const gridX = (x / rect.width) * mapData.width;
    const gridY = (y / rect.height) * mapData.height;
    const worldX = gridX * mapData.resolution + mapData.origin.x;
    const worldY = gridY * mapData.resolution + mapData.origin.y;
    return { x: worldX, y: worldY };
}

function renderWaypointList() {
    if (!navList) return;
    navList.innerHTML = '';
    if (waypoints.length === 0) {
        navStatus.textContent = 'No waypoints queued.';
        return;
    }
    navStatus.textContent = `${waypoints.length} waypoint(s) queued.`;
    waypoints.forEach((wp, idx) => {
        const item = document.createElement('div');
        item.className = 'nav-item';
        item.innerHTML = `<span>#${idx + 1}</span><span>${wp.x.toFixed(2)}, ${wp.y.toFixed(2)}</span>`;
        navList.appendChild(item);
    });
}

async function sendWaypoints(mode = 'replace') {
    if (waypoints.length === 0) {
        navStatus.textContent = 'Add at least one waypoint.';
        return;
    }
    try {
        const res = await fetch(`${AI_API_BASE}/navigation`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                waypoints: waypoints.map((wp) => ({ x: wp.x, y: wp.y })),
                mode: mode
            })
        });
        if (!res.ok) {
            navStatus.textContent = 'Failed to send waypoints.';
            return;
        }
        navStatus.textContent = mode === 'append' ? 'Waypoints appended.' : 'Waypoints sent.';
    } catch (err) {
        console.error('Navigation send failed', err);
        navStatus.textContent = 'AI navigation service unavailable.';
    }
}

async function clearNavigation() {
    try {
        await fetch(`${AI_API_BASE}/navigation/clear`, { method: 'POST' });
    } catch (_) {
        // ignore
    }
    waypoints = [];
    renderWaypointList();
    navStatus.textContent = 'Waypoints cleared.';
}

if (canvas) {
    canvas.addEventListener('click', (event) => {
        const world = canvasToWorld(event.clientX, event.clientY);
        if (!world) return;
        waypoints.push(world);
        renderWaypointList();
    });
}

if (navSendBtn) {
    navSendBtn.addEventListener('click', () => sendWaypoints('replace'));
}

if (navAppendBtn) {
    navAppendBtn.addEventListener('click', () => sendWaypoints('append'));
}

if (navClearBtn) {
    navClearBtn.addEventListener('click', clearNavigation);
}

setInterval(fetchFrame, 200);
setInterval(fetchMap, 2500);
fetchFrame();
fetchMap();
setStatus('Connecting…', false);
renderWaypointList();
