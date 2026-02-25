// API Configuration - Use current window location for API calls
const API_BASE = `${window.location.protocol}//${window.location.hostname}:3000/api`;
const AI_API_BASE = `${window.location.protocol}//${window.location.hostname}:3001/api/ai`;

// State
let currentGait = 'ripple';
let isDragging = false;
let currentJoystick = null;
let gamepadConnected = false;
let gamepadLayout = 'xbox'; // 'xbox' or 'playstation'
let gamepadIndex = null;
let gamepadAnimationFrame = null;
let aiChatSending = false;
let voiceRecording = false;
let voiceRecorder = null;
let voiceChunks = [];
let voiceStream = null;
let liveModeEnabled = false;
let liveRecording = false;
let liveRecorder = null;
let liveStream = null;
let liveAudioContext = null;
let liveScriptProcessor = null;
let liveWs = null;
let liveReconnectTimer = null;
let liveReconnectDelay = 500;
const LIVE_TARGET_SAMPLE_RATE = 16000;

function normalizeGaitName(name) {
    if (!name) return 'ripple';
    const lower = name.toLowerCase();
    if (lower.startsWith('tri')) return 'tripod';
    if (lower.startsWith('tet') || lower.startsWith('quad') || lower === 'bi') return 'tetrapod';
    if (lower.startsWith('wav')) return 'wave';
    if (lower.startsWith('rip')) return 'ripple';
    return lower;
}

function setActiveGaitButton(gaitName) {
    const gaitBtns = document.querySelectorAll('[data-gait]');
    gaitBtns.forEach(btn => {
        if (btn.dataset.gait === gaitName) {
            btn.classList.add('active');
        } else {
            btn.classList.remove('active');
        }
    });
}

async function syncCurrentGaitFromServer() {
    try {
        const res = await fetch(`${API_BASE}/gait`);
        if (!res.ok) return;
        const data = await res.json();
        const normalized = normalizeGaitName(data.current_gait);
        if (normalized && normalized !== currentGait) {
            currentGait = normalized;
            setActiveGaitButton(currentGait);
            syncGaitConfigUI(currentGait);
        }
        await loadGaitConfig(currentGait);
    } catch (_) {
        // ignore
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    initJoysticks();
    initGaitSelector();
    initPoseControls();
    initEmergencyStop();
    initAIChat();
    initTTS();
    initGamepad();
    initCustomGaitControls();
    initLegCalibration();
    initServoTweaks();
    startStatusUpdates();
});

// Joystick Control
function initJoysticks() {
    const moveJoystick = document.getElementById('move-joystick');
    const rotateJoystick = document.getElementById('rotate-joystick');

    if (!moveJoystick || !rotateJoystick) {
        return;
    }

    setupJoystick(moveJoystick, (x, y) => {
        // x = strafe (left/right), y = forward (forward/back)
        sendMoveCommand({ forward: y * 100, strafe: -x * 100, rotation: 0.0 });
    });

    setupJoystick(rotateJoystick, (x, y) => {
        // x = rotation
        sendMoveCommand({ forward: 0.0, strafe: 0.0, rotation: -x });
    });
}

function setupJoystick(joystick, callback) {
    const stick = joystick.querySelector('.joystick-stick');
    const radius = joystick.offsetWidth / 2;
    const stickRadius = stick.offsetWidth / 2;
    const maxDistance = radius - stickRadius;

    let isActive = false;
    let animationFrame = null;

    function handleStart(e) {
        e.preventDefault();
        isActive = true;
        stick.style.cursor = 'grabbing';
    }

    function handleMove(e) {
        if (!isActive) return;

        const rect = joystick.getBoundingClientRect();
        const centerX = rect.left + radius;
        const centerY = rect.top + radius;

        let clientX, clientY;
        if (e.type.includes('touch')) {
            clientX = e.touches[0].clientX;
            clientY = e.touches[0].clientY;
        } else {
            clientX = e.clientX;
            clientY = e.clientY;
        }

        let deltaX = clientX - centerX;
        let deltaY = clientY - centerY;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);

        if (distance > maxDistance) {
            const angle = Math.atan2(deltaY, deltaX);
            deltaX = Math.cos(angle) * maxDistance;
            deltaY = Math.sin(angle) * maxDistance;
        }

        stick.style.transform = `translate(calc(-50% + ${deltaX}px), calc(-50% + ${deltaY}px))`;

        // Normalize values to -1.0 to 1.0
        const normalizedX = deltaX / maxDistance;
        const normalizedY = -deltaY / maxDistance; // Invert Y axis

        if (animationFrame) {
            cancelAnimationFrame(animationFrame);
        }
        animationFrame = requestAnimationFrame(() => {
            callback(normalizedX, normalizedY);
        });
    }

    function handleEnd(e) {
        if (!isActive) return;
        e.preventDefault();
        isActive = false;
        stick.style.cursor = 'grab';
        stick.style.transform = 'translate(-50%, -50%)';

        if (animationFrame) {
            cancelAnimationFrame(animationFrame);
        }

        // Send zero velocity to stop the robot
        callback(0, 0);
    }

    // Mouse events
    stick.addEventListener('mousedown', handleStart);
    document.addEventListener('mousemove', handleMove);
    document.addEventListener('mouseup', handleEnd);

    // Touch events
    stick.addEventListener('touchstart', handleStart);
    document.addEventListener('touchmove', handleMove, { passive: false });
    document.addEventListener('touchend', handleEnd);
}

// Gait Selection
function initGaitSelector() {
    const gaitBtns = document.querySelectorAll('[data-gait]');
    if (!gaitBtns || gaitBtns.length === 0) {
        return;
    }
    gaitBtns.forEach(btn => {
        if (btn.disabled) {
            return;
        }
        btn.addEventListener('click', () => {
            currentGait = btn.dataset.gait;
            setActiveGaitButton(currentGait);
            setGait(currentGait);
            syncGaitConfigUI(currentGait);
            loadGaitConfig(currentGait);
        });
    });

    // Set default active
    const defaultGait = document.querySelector('[data-gait="ripple"]');
    if (defaultGait) {
        defaultGait.classList.add('active');
    }
}

// Pose Controls
function initPoseControls() {
    const sliders = {
        x: document.getElementById('pose-x'),
        y: document.getElementById('pose-y'),
        z: document.getElementById('pose-z'),
        roll: document.getElementById('pose-roll'),
        pitch: document.getElementById('pose-pitch'),
        yaw: document.getElementById('pose-yaw')
    };

    const values = {
        x: document.getElementById('pose-x-val'),
        y: document.getElementById('pose-y-val'),
        z: document.getElementById('pose-z-val'),
        roll: document.getElementById('pose-roll-val'),
        pitch: document.getElementById('pose-pitch-val'),
        yaw: document.getElementById('pose-yaw-val')
    };

    const sliderKeys = Object.keys(sliders).filter(key => sliders[key]);
    if (sliderKeys.length === 0) {
        return;
    }

    let poseUpdateTimeout = null;

    sliderKeys.forEach(key => {
        sliders[key].addEventListener('input', (e) => {
            const value = parseFloat(e.target.value);
            values[key].textContent = value.toFixed(2);

            // Debounce pose updates
            clearTimeout(poseUpdateTimeout);
            poseUpdateTimeout = setTimeout(() => {
                // API only accepts roll, pitch, yaw (not x, y, z position)
                const pose = {
                    roll: parseFloat(sliders.roll.value),
                    pitch: parseFloat(sliders.pitch.value),
                    yaw: parseFloat(sliders.yaw.value)
                };
                setPose(pose);
            }, 100);
        });
    });

    // Emit initial pose for the visualizer
    if (sliders.roll && sliders.pitch && sliders.yaw) {
        const initialPose = {
            roll: parseFloat(sliders.roll.value),
            pitch: parseFloat(sliders.pitch.value),
            yaw: parseFloat(sliders.yaw.value)
        };
        window.dispatchEvent(new CustomEvent('hexapod:pose', { detail: initialPose }));
    }
}

// Emergency Stop
function initEmergencyStop() {
    const stopBtn = document.getElementById('emergency-stop');
    if (!stopBtn) {
        return;
    }
    stopBtn.addEventListener('click', () => {
        emergencyStop();
    });
}

// Text-to-Speech
function initTTS() {
    const speakBtn = document.getElementById('speak-btn');
    const ttsInput = document.getElementById('tts-input');
    const ttsStatus = document.getElementById('tts-status');

    if (!speakBtn || !ttsInput || !ttsStatus) {
        return;
    }

    // Speak button click
    speakBtn.addEventListener('click', () => {
        const text = ttsInput.value.trim();
        if (text.length === 0) {
            showTTSStatus('Please enter some text', 'error');
            return;
        }
        speakText(text);
    });

    // Enter key in input field
    ttsInput.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') {
            const text = ttsInput.value.trim();
            if (text.length > 0) {
                speakText(text);
            }
        }
    });
}

async function speakText(text) {
    const voice = document.getElementById('tts-voice').value;
    const ttsStatus = document.getElementById('tts-status');

    try {
        showTTSStatus('Speaking...', 'loading');

        const response = await fetch(`${API_BASE}/tts`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                text: text,
                voice: voice
            })
        });

        if (response.ok) {
            const data = await response.json();
            showTTSStatus('✓ Sent to speaker', 'success');
            console.log('TTS:', data.message);

            // Clear status after 2 seconds
            setTimeout(() => {
                ttsStatus.textContent = '';
                ttsStatus.className = 'tts-status';
            }, 2000);
        } else {
            showTTSStatus('Failed to speak', 'error');
        }
    } catch (error) {
        console.error('Error sending TTS request:', error);
        showTTSStatus('Connection error', 'error');
        updateConnectionStatus(false);
    }
}

function showTTSStatus(message, type) {
    const ttsStatus = document.getElementById('tts-status');
    ttsStatus.textContent = message;
    ttsStatus.className = `tts-status ${type}`;
}

// API Calls
async function sendMoveCommand(velocity) {
    window.dispatchEvent(new CustomEvent('hexapod:move', { detail: velocity }));
    try {
        console.log('Sending move command:', velocity);
        const response = await fetch(`${API_BASE}/move`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(velocity)
        });
        if (!response.ok) {
            console.error('Failed to send move command');
        }
    } catch (error) {
        console.error('Error sending move command:', error);
        updateConnectionStatus(false);
    }
}

async function setGait(gait) {
    try {
        const response = await fetch(`${API_BASE}/gait`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ gait_name: gait })
        });
        if (response.ok) {
            const data = await response.json();
            currentGait = normalizeGaitName(data.current_gait);
            setActiveGaitButton(currentGait);
            syncGaitConfigUI(currentGait);
            loadGaitConfig(currentGait);
            console.log(`Gait changed to: ${data.current_gait}`);
            window.dispatchEvent(new CustomEvent('hexapod:status', {
                detail: { gait_phase: 0, gait_name: data.current_gait }
            }));
        }
    } catch (error) {
        console.error('Error setting gait:', error);
        updateConnectionStatus(false);
    }
}

function syncGaitConfigUI(gaitName) {
    const nameInput = document.getElementById('custom-gait-name');
    if (nameInput) {
        nameInput.value = gaitName;
    }
}

async function setPose(pose) {
    try {
        // API expects only roll, pitch, yaw (not x, y, z)
        const poseData = {
            roll: pose.roll || 0,
            pitch: pose.pitch || 0,
            yaw: pose.yaw || 0
        };
        window.dispatchEvent(new CustomEvent('hexapod:pose', { detail: poseData }));
        const response = await fetch(`${API_BASE}/pose`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(poseData)
        });
        if (!response.ok) {
            console.error('Failed to set pose');
        }
    } catch (error) {
        console.error('Error setting pose:', error);
        updateConnectionStatus(false);
    }
}

async function emergencyStop() {
    try {
        const response = await fetch(`${API_BASE}/estop`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({})
        });
        if (response.ok) {
            console.log('Emergency stop activated');
            // Reset all controls
            document.querySelectorAll('.joystick-stick').forEach(stick => {
                stick.style.transform = 'translate(-50%, -50%)';
            });
        }
    } catch (error) {
        console.error('Error sending emergency stop:', error);
        updateConnectionStatus(false);
    }
}

// Custom Gait UI
function initCustomGaitControls() {
    const ids = [
        'push-fraction', 'gait-speed', 'step-length', 'step-height', 'base-height', 'push-gain', 'max-step', 'max-speed',
        'off-lf', 'off-lm', 'off-lb', 'off-rf', 'off-rm', 'off-rb'
    ];
    ids.forEach(id => {
        const el = document.getElementById(id);
        const val = document.getElementById(id + '-val');
        if (el && val) {
            el.addEventListener('input', () => {
                const raw = parseFloat(el.value);
                let precision = 2;
                if (id.startsWith('off-') || id === 'push-fraction') {
                    precision = 3;
                }
                if (['step-length', 'step-height', 'base-height', 'max-step', 'max-speed'].includes(id)) {
                    precision = 0;
                }
                val.textContent = Number.isFinite(raw) ? raw.toFixed(precision) : el.value;
            });
        }
    });

    const applyBtn = document.getElementById('apply-custom-gait');
    if (!applyBtn) return;

    // Mode Buttons
    const normalBtn = document.getElementById('mode-normal');
    const turboBtn = document.getElementById('mode-turbo');

    if (normalBtn) {
        normalBtn.addEventListener('click', () => {
            document.getElementById('step-length').value = 80;
            document.getElementById('step-height').value = 70;
            document.getElementById('gait-speed').value = 1.0;
            // Trigger input events to update labels
            ['step-length', 'step-height', 'gait-speed'].forEach(id => {
                document.getElementById(id).dispatchEvent(new Event('input'));
            });
            applyBtn.click();
        });
    }

    if (turboBtn) {
        turboBtn.addEventListener('click', () => {
            document.getElementById('step-length').value = 110;
            document.getElementById('step-height').value = 50;
            document.getElementById('gait-speed').value = 1.3;
            // Trigger input events to update labels
            ['step-length', 'step-height', 'gait-speed'].forEach(id => {
                document.getElementById(id).dispatchEvent(new Event('input'));
            });
            applyBtn.click();
        });
    }

    const nameInput = document.getElementById('custom-gait-name');
    if (nameInput) {
        nameInput.value = currentGait;
        nameInput.readOnly = true;
    }

    syncCurrentGaitFromServer();

    applyBtn.addEventListener('click', async () => {
        const payload = {
            gait_name: currentGait,
            config: {
                duty_factor: parseFloat(document.getElementById('push-fraction').value),
                speed: parseFloat(document.getElementById('gait-speed').value),
                step_length_mm: parseFloat(document.getElementById('step-length').value),
                step_height_mm: parseFloat(document.getElementById('step-height').value),
                base_height_mm: parseFloat(document.getElementById('base-height').value),
                body_push_gain: parseFloat(document.getElementById('push-gain').value),
                phase_offsets: [
                    parseFloat(document.getElementById('off-lf').value),
                    parseFloat(document.getElementById('off-lm').value),
                    parseFloat(document.getElementById('off-lb').value),
                    parseFloat(document.getElementById('off-rf').value),
                    parseFloat(document.getElementById('off-rm').value),
                    parseFloat(document.getElementById('off-rb').value)
                ],
                max_step_length: parseFloat(document.getElementById('max-step').value),
                max_speed: parseFloat(document.getElementById('max-speed').value)
            }
        };

        try {
            const res = await fetch(`${API_BASE}/gait_config`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload)
            });
            if (res.ok) {
                const data = await res.json();
                currentGait = normalizeGaitName(data.gait_name);
                setActiveGaitButton(currentGait);
                syncGaitConfigUI(currentGait);
                await loadGaitConfig(currentGait);
                console.log('Gait config applied:', data.gait_name);
            } else {
                console.error('Failed to apply gait config', res.status);
                updateConnectionStatus(false);
            }
        } catch (err) {
            console.error('Error applying gait config', err);
            updateConnectionStatus(false);
        }
    });
}

async function loadGaitConfig(gaitName) {
    try {
        const res = await fetch(`${API_BASE}/gait_config?gait_name=${encodeURIComponent(gaitName)}`);
        if (!res.ok) return;
        const data = await res.json();
        const cfg = data.config;

        const setVal = (id, value, precision = 2) => {
            const el = document.getElementById(id);
            const val = document.getElementById(`${id}-val`);
            if (!el) return;
            el.value = value;
            if (val) val.textContent = value.toFixed(precision);
        };

        setVal('push-fraction', cfg.duty_factor, 3);
        setVal('gait-speed', cfg.speed, 2);
        setVal('step-length', cfg.step_length_mm, 0);
        setVal('step-height', cfg.step_height_mm, 0);
        setVal('base-height', cfg.base_height_mm, 0);
        setVal('push-gain', cfg.body_push_gain, 2);
        if (cfg.max_step_length && cfg.max_step_length > 0) {
            setVal('max-step', cfg.max_step_length, 0);
        }
        if (cfg.max_speed && cfg.max_speed > 0) {
            setVal('max-speed', cfg.max_speed, 0);
        }

        if (Array.isArray(cfg.phase_offsets)) {
            const offsets = cfg.phase_offsets;
            setVal('off-lf', offsets[0] || 0, 3);
            setVal('off-lm', offsets[1] || 0, 3);
            setVal('off-lb', offsets[2] || 0, 3);
            setVal('off-rf', offsets[3] || 0, 3);
            setVal('off-rm', offsets[4] || 0, 3);
            setVal('off-rb', offsets[5] || 0, 3);
        }
    } catch (_) {
        // ignore
    }
}

async function updateStatus() {
    try {
        const [statusRes, batteryRes] = await Promise.all([
            fetch(`${API_BASE}/status`),
            fetch(`${API_BASE}/battery`)
        ]);

        if (statusRes.ok && batteryRes.ok) {
            const status = await statusRes.json();
            const battery = await batteryRes.json();

            // Update status display
            const gaitStatus = document.getElementById('gait-status');
            const stateStatus = document.getElementById('state-status');
            if (gaitStatus) gaitStatus.textContent = status.gait_name || 'unknown';
            if (stateStatus) stateStatus.textContent = battery.power_state || 'unknown';

            if (status.gait_name) {
                const normalized = normalizeGaitName(status.gait_name);
                if (normalized !== currentGait) {
                    currentGait = normalized;
                    setActiveGaitButton(currentGait);
                    syncGaitConfigUI(currentGait);
                    loadGaitConfig(currentGait);
                }
            }

            // Update battery display; if backend has no data, surface it clearly
            const voltageValue = document.getElementById('voltage-value');
            const currentValue = document.getElementById('current-value');
            if (voltageValue && currentValue) {
                if (battery.has_data) {
                    voltageValue.textContent = `${battery.voltage.toFixed(2)}V`;
                    currentValue.textContent = `${battery.current.toFixed(2)}A`;
                } else {
                    voltageValue.textContent = 'N/A';
                    currentValue.textContent = 'N/A';
                }
            }

            window.dispatchEvent(new CustomEvent('hexapod:status', {
                detail: {
                    gait_phase: status.gait_phase || 0,
                    gait_name: status.gait_name || 'unknown',
                    power_state: battery.power_state || 'unknown'
                }
            }));

            updateConnectionStatus(true);
        } else {
            updateConnectionStatus(false);
        }
        await updateImu();
    } catch (error) {
        console.error('Error fetching status:', error);
        updateConnectionStatus(false);
        await updateImu();
    }
}

async function updateImu() {
    const setImuValue = (id, value) => {
        const el = document.getElementById(id);
        if (el) el.textContent = value;
    };

    try {
        const imuRes = await fetch(`${API_BASE}/imu`);
        if (!imuRes.ok) {
            setImuValue('imu-roll', 'N/A');
            setImuValue('imu-pitch', 'N/A');
            setImuValue('imu-yaw', 'N/A');
            setImuValue('imu-calibration', 'N/A');
            setImuValue('imu-quat', 'N/A');
            return;
        }

        const imu = await imuRes.json();
        if (!imu.success) {
            setImuValue('imu-roll', 'N/A');
            setImuValue('imu-pitch', 'N/A');
            setImuValue('imu-yaw', 'N/A');
            setImuValue('imu-calibration', 'N/A');
            setImuValue('imu-quat', 'N/A');
            return;
        }

        const fmt = (val) => Number(val).toFixed(2);
        setImuValue('imu-roll', `${fmt(imu.euler[0])}°`);
        setImuValue('imu-pitch', `${fmt(imu.euler[1])}°`);
        setImuValue('imu-yaw', `${fmt(imu.euler[2])}°`);
        setImuValue('imu-calibration', `${imu.calibration}/3`);
        setImuValue(
            'imu-quat',
            `${fmt(imu.quat[0])}, ${fmt(imu.quat[1])}, ${fmt(imu.quat[2])}, ${fmt(imu.quat[3])}`
        );
    } catch (_) {
        setImuValue('imu-roll', 'N/A');
        setImuValue('imu-pitch', 'N/A');
        setImuValue('imu-yaw', 'N/A');
        setImuValue('imu-calibration', 'N/A');
        setImuValue('imu-quat', 'N/A');
    }
}

function updateConnectionStatus(connected) {
    const statusElement = document.getElementById('connection-status');
    if (!statusElement) {
        return;
    }
    if (connected) {
        statusElement.className = 'connection-status connected';
        statusElement.textContent = '● Connected';
    } else {
        statusElement.className = 'connection-status disconnected';
        statusElement.textContent = '● Disconnected';
    }
}

function startStatusUpdates() {
    updateStatus(); // Initial update
    setInterval(updateStatus, 1000); // Update every second
}

// ===== GAMEPAD API SUPPORT =====

function initGamepad() {
    window.addEventListener('gamepadconnected', (e) => {
        console.log('Gamepad connected:', e.gamepad.id);
        gamepadConnected = true;
        gamepadIndex = e.gamepad.index;
        updateGamepadStatus(true, e.gamepad.id);
        startGamepadPolling();
    });

    window.addEventListener('gamepaddisconnected', (e) => {
        console.log('Gamepad disconnected');
        gamepadConnected = false;
        gamepadIndex = null;
        updateGamepadStatus(false);
        stopGamepadPolling();
    });

    // Check for already connected gamepads
    const gamepads = navigator.getGamepads();
    for (let i = 0; i < gamepads.length; i++) {
        if (gamepads[i]) {
            gamepadConnected = true;
            gamepadIndex = i;
            updateGamepadStatus(true, gamepads[i].id);
            startGamepadPolling();
            break;
        }
    }
}

function updateGamepadStatus(connected, gamepadName = '') {
    const statusElement = document.getElementById('gamepadStatus');
    const statusText = document.getElementById('gamepadStatusText');
    const infoElement = document.getElementById('gamepadInfo');

    if (connected) {
        statusElement.classList.add('connected');
        statusText.textContent = `Connected: ${gamepadName}`;
        infoElement.innerHTML = `
            <strong>Controls:</strong><br>
            Left Stick: Move forward/backward & strafe left/right<br>
            Right Stick: Rotate left/right<br>
            D-Pad: Quick body pose adjustments<br>
            A/X: Emergency Stop | B/Circle: Center pose
        `;
    } else {
        statusElement.classList.remove('connected');
        statusText.textContent = 'No gamepad connected';
        infoElement.innerHTML = '<small>Connect a gamepad and press any button to activate</small>';
    }
}

function setGamepadLayout(layout) {
    gamepadLayout = layout;

    // Update button states
    const xboxBtn = document.getElementById('xboxBtn');
    const psBtn = document.getElementById('playstationBtn');

    if (layout === 'xbox') {
        xboxBtn.classList.add('active');
        psBtn.classList.remove('active');
    } else {
        psBtn.classList.add('active');
        xboxBtn.classList.remove('active');
    }

    console.log(`Gamepad layout set to: ${layout}`);
}

// Gamepad button mappings
const GAMEPAD_MAPPING = {
    xbox: {
        A: 0,           // Bottom button (Emergency Stop)
        B: 1,           // Right button (Center Pose)
        X: 2,           // Left button
        Y: 3,           // Top button
        LB: 4,          // Left bumper
        RB: 5,          // Right bumper
        LT: 6,          // Left trigger
        RT: 7,          // Right trigger
        SELECT: 8,      // Select/Back
        START: 9,       // Start
        LS: 10,         // Left stick button
        RS: 11,         // Right stick button
        DPAD_UP: 12,
        DPAD_DOWN: 13,
        DPAD_LEFT: 14,
        DPAD_RIGHT: 15
    },
    playstation: {
        X: 0,           // Cross (Emergency Stop)
        CIRCLE: 1,      // Circle (Center Pose)
        SQUARE: 2,      // Square
        TRIANGLE: 3,    // Triangle
        L1: 4,          // L1
        R1: 5,          // R1
        L2: 6,          // L2
        R2: 7,          // R2
        SHARE: 8,       // Share
        OPTIONS: 9,     // Options
        L3: 10,         // L3 (Left stick button)
        R3: 11,         // R3 (Right stick button)
        DPAD_UP: 12,
        DPAD_DOWN: 13,
        DPAD_LEFT: 14,
        DPAD_RIGHT: 15
    }
};

let previousButtonStates = {};
let previousDPadStates = {};

function startGamepadPolling() {
    if (gamepadAnimationFrame) {
        cancelAnimationFrame(gamepadAnimationFrame);
    }
    pollGamepad();
}

function stopGamepadPolling() {
    if (gamepadAnimationFrame) {
        cancelAnimationFrame(gamepadAnimationFrame);
        gamepadAnimationFrame = null;
    }
}

function pollGamepad() {
    if (!gamepadConnected || gamepadIndex === null) {
        return;
    }

    const gamepad = navigator.getGamepads()[gamepadIndex];
    if (!gamepad) {
        gamepadConnected = false;
        updateGamepadStatus(false);
        return;
    }

    // Get current button mapping
    const mapping = GAMEPAD_MAPPING[gamepadLayout];

    // Handle analog sticks
    const leftStickX = gamepad.axes[0];  // Left/Right
    const leftStickY = gamepad.axes[1];  // Forward/Back
    const rightStickX = gamepad.axes[2]; // Rotation

    // Apply deadzone
    const deadzone = 0.15;
    const processAxis = (value) => {
        return Math.abs(value) < deadzone ? 0 : value;
    };

    const strafeX = -processAxis(leftStickX);
    const forwardY = -processAxis(leftStickY); // Invert Y axis
    const rotation = -processAxis(rightStickX);

    // Send movement command (convert -1 to 1 range to -100 to 100 mm/s)
    // Always send commands, even when zero, to ensure robot stops when sticks are released
    sendMoveCommand({
        forward: forwardY * 100,
        strafe: strafeX * 100,
        rotation: rotation
    });

    // Handle button presses (detect button down events)
    const buttonA = gamepad.buttons[mapping.A || 0];
    const buttonB = gamepad.buttons[mapping.B || 1];

    // Emergency Stop (A/X button)
    if (buttonA && buttonA.pressed && !previousButtonStates.A) {
        emergencyStop();
    }
    previousButtonStates.A = buttonA ? buttonA.pressed : false;

    // Center Pose (B/Circle button)
    if (buttonB && buttonB.pressed && !previousButtonStates.B) {
        centerPose();
    }
    previousButtonStates.B = buttonB ? buttonB.pressed : false;

    // D-Pad for body pose adjustments
    const dpadUp = gamepad.buttons[mapping.DPAD_UP];
    const dpadDown = gamepad.buttons[mapping.DPAD_DOWN];
    const dpadLeft = gamepad.buttons[mapping.DPAD_LEFT];
    const dpadRight = gamepad.buttons[mapping.DPAD_RIGHT];

    const poseStep = 2; // degrees per press

    if (dpadUp && dpadUp.pressed && !previousDPadStates.UP) {
        adjustPose('pitch', poseStep);
    }
    previousDPadStates.UP = dpadUp ? dpadUp.pressed : false;

    if (dpadDown && dpadDown.pressed && !previousDPadStates.DOWN) {
        adjustPose('pitch', -poseStep);
    }
    previousDPadStates.DOWN = dpadDown ? dpadDown.pressed : false;

    if (dpadLeft && dpadLeft.pressed && !previousDPadStates.LEFT) {
        adjustPose('roll', -poseStep);
    }
    previousDPadStates.LEFT = dpadLeft ? dpadLeft.pressed : false;

    if (dpadRight && dpadRight.pressed && !previousDPadStates.RIGHT) {
        adjustPose('roll', poseStep);
    }
    previousDPadStates.RIGHT = dpadRight ? dpadRight.pressed : false;

    // Continue polling
    gamepadAnimationFrame = requestAnimationFrame(pollGamepad);
}

function centerPose() {
    const sliders = {
        x: document.getElementById('pose-x'),
        y: document.getElementById('pose-y'),
        z: document.getElementById('pose-z'),
        roll: document.getElementById('pose-roll'),
        pitch: document.getElementById('pose-pitch'),
        yaw: document.getElementById('pose-yaw')
    };

    const values = {
        x: document.getElementById('pose-x-val'),
        y: document.getElementById('pose-y-val'),
        z: document.getElementById('pose-z-val'),
        roll: document.getElementById('pose-roll-val'),
        pitch: document.getElementById('pose-pitch-val'),
        yaw: document.getElementById('pose-yaw-val')
    };

    // Reset all sliders to 0
    Object.keys(sliders).forEach(key => {
        if (sliders[key]) {
            sliders[key].value = 0;
            if (values[key]) {
                values[key].textContent = '0.00';
            }
        }
    });

    // Send centered pose to robot (API only accepts roll, pitch, yaw)
    setPose({ roll: 0, pitch: 0, yaw: 0 });
    console.log('Pose centered');
}

function adjustPose(axis, delta) {
    const slider = document.getElementById(`pose-${axis}`);
    const valueDisplay = document.getElementById(`pose-${axis}-val`);

    if (!slider) return;

    let currentValue = parseFloat(slider.value);
    let newValue = currentValue + delta;

    // Clamp to slider min/max
    const min = parseFloat(slider.min);
    const max = parseFloat(slider.max);
    newValue = Math.max(min, Math.min(max, newValue));

    slider.value = newValue;
    if (valueDisplay) {
        valueDisplay.textContent = newValue.toFixed(2);
    }

    // Send updated pose (API only accepts roll, pitch, yaw)
    const pose = {
        roll: parseFloat(document.getElementById('pose-roll').value),
        pitch: parseFloat(document.getElementById('pose-pitch').value),
        yaw: parseFloat(document.getElementById('pose-yaw').value)
    };
    setPose(pose);
}

// Make setGamepadLayout available globally
window.setGamepadLayout = setGamepadLayout;

// ===== LEG CALIBRATION =====

function initLegCalibration() {
    const legIds = ['lf', 'lm', 'lb', 'rf', 'rm', 'rb'];
    const axes = ['x', 'y', 'z'];
    let legStanceAbortController = null;
    const stanceRanges = {
        x: { min: -200, max: 200, step: 1 },
        y: { min: -200, max: 200, step: 1 },
        z: { min: -200, max: 0, step: 1 }
    };

    if (!document.getElementById('lf-x')) {
        return;
    }

    // Set up slider value displays
    legIds.forEach(legId => {
        axes.forEach(axis => {
            const slider = document.getElementById(`${legId}-${axis}`);
            const valueDisplay = document.getElementById(`${legId}-${axis}-val`);

            if (slider && valueDisplay) {
                const range = stanceRanges[axis];
                if (range) {
                    slider.min = range.min;
                    slider.max = range.max;
                    slider.step = range.step;
                }
                slider.addEventListener('input', () => {
                    valueDisplay.textContent = parseFloat(slider.value).toFixed(1);
                    // Instant live apply while dragging (autosaves on server)
                    applyLegStanceLive();
                });
            }
        });
    });

    // Load current stance button
    const loadBtn = document.getElementById('load-current-stance');
    if (loadBtn) {
        loadBtn.addEventListener('click', loadCurrentStance);
    }

    // Reset button
    const resetBtn = document.getElementById('reset-leg-stance');
    if (resetBtn) {
        resetBtn.addEventListener('click', resetLegStance);
    }

    // Load current stance on init so sliders reflect actual defaults
    loadCurrentStance();
}

async function loadCurrentStance() {
    try {
        const response = await fetch(`${API_BASE}/leg_stance`);
        if (response.ok) {
            const data = await response.json();
            const stance = data.current_stance;

            // Update sliders with current values
            setLegSliders('lf', stance.left_front);
            setLegSliders('lm', stance.left_middle);
            setLegSliders('lb', stance.left_back);
            setLegSliders('rf', stance.right_front);
            setLegSliders('rm', stance.right_middle);
            setLegSliders('rb', stance.right_back);

            console.log('Loaded current stance:', stance);
        } else {
            console.error('Failed to load current stance');
        }
    } catch (error) {
        console.error('Error loading current stance:', error);
        updateConnectionStatus(false);
    }
}

function setLegSliders(legId, values) {
    const axes = ['x', 'y', 'z'];
    axes.forEach((axis, index) => {
        const slider = document.getElementById(`${legId}-${axis}`);
        const valueDisplay = document.getElementById(`${legId}-${axis}-val`);

        if (slider && valueDisplay) {
            const min = parseFloat(slider.min);
            const max = parseFloat(slider.max);
            const raw = parseFloat(values[index]);
            const clamped = Number.isNaN(raw) ? 0 : Math.min(max, Math.max(min, raw));
            slider.value = clamped;
            valueDisplay.textContent = clamped.toFixed(1);
        }
    });
}

function collectLegStancePayload() {
    return {
        left_front: [
            parseFloat(document.getElementById('lf-x').value),
            parseFloat(document.getElementById('lf-y').value),
            parseFloat(document.getElementById('lf-z').value)
        ],
        left_middle: [
            parseFloat(document.getElementById('lm-x').value),
            parseFloat(document.getElementById('lm-y').value),
            parseFloat(document.getElementById('lm-z').value)
        ],
        left_back: [
            parseFloat(document.getElementById('lb-x').value),
            parseFloat(document.getElementById('lb-y').value),
            parseFloat(document.getElementById('lb-z').value)
        ],
        right_front: [
            parseFloat(document.getElementById('rf-x').value),
            parseFloat(document.getElementById('rf-y').value),
            parseFloat(document.getElementById('rf-z').value)
        ],
        right_middle: [
            parseFloat(document.getElementById('rm-x').value),
            parseFloat(document.getElementById('rm-y').value),
            parseFloat(document.getElementById('rm-z').value)
        ],
        right_back: [
            parseFloat(document.getElementById('rb-x').value),
            parseFloat(document.getElementById('rb-y').value),
            parseFloat(document.getElementById('rb-z').value)
        ]
    };
}

async function applyLegStance() {
    const payload = collectLegStancePayload();

    try {
        const response = await fetch(`${API_BASE}/leg_stance`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });

        if (response.ok) {
            const data = await response.json();
            console.log('✓ Leg stance applied and saved:', data.message);
            alert('✓ Leg stance applied and saved as default.');
        } else {
            console.error('Failed to apply leg stance');
            alert('Failed to apply leg stance');
        }
    } catch (error) {
        console.error('Error applying leg stance:', error);
        updateConnectionStatus(false);
        alert('Connection error while applying leg stance');
    }
}

async function applyLegStanceLive() {
    const payload = collectLegStancePayload();
    try {
        // Abort previous in-flight request to avoid backlog
        if (window.__legStanceAbortController) {
            try { window.__legStanceAbortController.abort(); } catch (e) { }
        }
        window.__legStanceAbortController = new AbortController();

        await fetch(`${API_BASE}/leg_stance`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
            signal: window.__legStanceAbortController.signal
        });
    } catch (e) {
        // ignore
    }
}

async function saveLegStanceAsDefault() {
    const payload = collectLegStancePayload();
    try {
        const response = await fetch(`${API_BASE}/leg_stance/save`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });
        if (response.ok) {
            console.log('✓ Leg stance saved as default');
            alert('✓ Saved as default. Will be loaded automatically on next start.');
        } else {
            console.error('Failed to save leg stance');
            alert('Failed to save leg stance');
        }
    } catch (e) {
        console.error('Error saving leg stance:', e);
        alert('Connection error while saving leg stance');
    }
}

function resetLegStance() {
    // Reset to default values from LegStances::default()
    setLegSliders('lf', [0.0, -45.0, -70.0]);
    setLegSliders('lm', [0.0, -55.0, -50.0]);
    setLegSliders('lb', [0.0, -45.0, -70.0]);
    setLegSliders('rf', [0.0, 45.0, -70.0]);
    setLegSliders('rm', [0.0, 55.0, -50.0]);
    setLegSliders('rb', [0.0, 45.0, -70.0]);

    console.log('Reset to default stance');
    applyLegStanceLive();
}

// ===== SERVO ANGLE TWEAKS =====

const SERVO_TWEAK_MIN = -90;
const SERVO_TWEAK_MAX = 90;
const SERVO_TWEAK_STEP = 0.5;
const LOCKABLE_PARTS = ['coxa', 'femur', 'tibia'];
const LOCK_SERVO_TWEAKS_UI = false;

function getAxisLockButton(legId, part) {
    return document.getElementById(`${legId}-${part}-lock`);
}

function isAxisLocked(legId, part) {
    const btn = getAxisLockButton(legId, part);
    return !!btn && btn.dataset.locked === 'true';
}

function setAxisLocked(legId, part, locked) {
    const btn = getAxisLockButton(legId, part);
    const slider = document.getElementById(`${legId}-${part}`);
    const numberInput = document.getElementById(`${legId}-${part}-input`);
    if (!btn || !slider) return;

    btn.dataset.locked = locked ? 'true' : 'false';
    btn.textContent = locked ? '🔒' : '🔓';
    btn.classList.toggle('locked', locked);
    slider.disabled = locked;
    if (numberInput) numberInput.disabled = locked;
}

function clampServoTweak(value) {
    const num = parseFloat(value);
    if (Number.isNaN(num)) return 0;
    return Math.min(SERVO_TWEAK_MAX, Math.max(SERVO_TWEAK_MIN, num));
}

function normalizeServoTweak(value) {
    const clamped = clampServoTweak(value);
    return Math.round(clamped / SERVO_TWEAK_STEP) * SERVO_TWEAK_STEP;
}

function initServoTweaks() {
    const legIds = ['lf', 'lm', 'lb', 'rf', 'rm', 'rb'];
    const parts = ['coxa', 'femur', 'tibia'];

    if (!document.getElementById('lf-coxa')) {
        return;
    }

    // Bind slider inputs
    legIds.forEach(legId => {
        parts.forEach(part => {
            const slider = document.getElementById(`${legId}-${part}`);
            const val = document.getElementById(`${legId}-${part}-val`);
            if (!slider || !val) return;

            // Widen range to ±90 and add numeric entry alongside slider
            slider.min = SERVO_TWEAK_MIN;
            slider.max = SERVO_TWEAK_MAX;
            slider.step = SERVO_TWEAK_STEP;

            let numberInput = document.getElementById(`${legId}-${part}-input`);
            if (!numberInput) {
                numberInput = document.createElement('input');
                numberInput.type = 'number';
                numberInput.id = `${legId}-${part}-input`;
                numberInput.min = SERVO_TWEAK_MIN;
                numberInput.max = SERVO_TWEAK_MAX;
                numberInput.step = SERVO_TWEAK_STEP;
                numberInput.value = slider.value;

                const row = document.createElement('div');
                row.className = 'slider-input-row';
                const container = slider.parentElement;
                row.appendChild(slider);
                row.appendChild(numberInput);
                if (container) {
                    container.appendChild(row);
                }
            }

            const updateValue = (raw, pushUpdate = true) => {
                const normalized = normalizeServoTweak(raw);
                slider.value = normalized;
                numberInput.value = normalized;
                val.textContent = normalized.toFixed(1);
                if (pushUpdate) applyServoTweaksLive();
            };

            if (LOCKABLE_PARTS.includes(part)) {
                if (LOCK_SERVO_TWEAKS_UI) {
                    slider.disabled = true;
                    numberInput.disabled = true;
                } else {
                    const group = slider.closest('.slider-group');
                    const label = group ? group.querySelector('label') : null;
                    if (label && !getAxisLockButton(legId, part)) {
                        label.classList.add('servo-axis-label');
                        const lockBtn = document.createElement('button');
                        lockBtn.type = 'button';
                        lockBtn.id = `${legId}-${part}-lock`;
                        lockBtn.className = 'axis-lock-btn';
                        lockBtn.dataset.locked = 'false';
                        lockBtn.textContent = '🔓';
                        lockBtn.addEventListener('click', () => {
                            const nextLocked = !isAxisLocked(legId, part);
                            setAxisLocked(legId, part, nextLocked);
                            if (nextLocked) applyServoTweaksLive();
                        });
                        label.appendChild(lockBtn);
                    }
                }
            }

            slider.addEventListener('input', () => updateValue(slider.value));
            numberInput.addEventListener('input', () => updateValue(numberInput.value));

            // Ensure initial display reflects the normalized range
            updateValue(slider.value, false);
        });
    });

    // Buttons
    const loadBtn = document.getElementById('load-servo-tweaks');
    if (loadBtn) loadBtn.addEventListener('click', loadCurrentServoTweaks);
    const saveBtn = document.getElementById('save-servo-tweaks');
    if (saveBtn) {
        if (LOCK_SERVO_TWEAKS_UI) {
            saveBtn.disabled = true;
        } else {
            saveBtn.addEventListener('click', saveServoTweaksNow);
        }
    }
    const resetBtn = document.getElementById('reset-servo-tweaks');
    if (resetBtn) {
        if (LOCK_SERVO_TWEAKS_UI) {
            resetBtn.disabled = true;
        } else {
            resetBtn.addEventListener('click', resetServoTweaks);
        }
    }

    // Load current from server on init
    loadCurrentServoTweaks();

    // Save on navigation to avoid losing recent tweaks.
    window.addEventListener('beforeunload', () => {
        const payload = collectServoTweaksPayload();
        scheduleServoTweaksSave(payload);
    });
}

function collectServoTweaksPayload() {
    const getTriplet = (id) => [
        parseFloat(document.getElementById(`${id}-coxa`).value),
        parseFloat(document.getElementById(`${id}-femur`).value),
        parseFloat(document.getElementById(`${id}-tibia`).value),
    ];

    return {
        left_front: getTriplet('lf'),
        left_middle: getTriplet('lm'),
        left_back: getTriplet('lb'),
        right_front: getTriplet('rf'),
        right_middle: getTriplet('rm'),
        right_back: getTriplet('rb'),
    };
}

function setServoSliders(legId, values) {
    const parts = ['coxa', 'femur', 'tibia'];
    parts.forEach((part, index) => {
        const slider = document.getElementById(`${legId}-${part}`);
        const val = document.getElementById(`${legId}-${part}-val`);
        const numberInput = document.getElementById(`${legId}-${part}-input`);
        if (!slider || !val) return;
        if (LOCKABLE_PARTS.includes(part) && isAxisLocked(legId, part)) return;

        const normalized = normalizeServoTweak(values[index]);
        slider.value = normalized;
        if (numberInput) numberInput.value = normalized;
        val.textContent = normalized.toFixed(1);
    });
}

async function loadCurrentServoTweaks() {
    try {
        const res = await fetch(`${API_BASE}/servo_tweaks`);
        if (res.ok) {
            const data = await res.json();
            const t = data.tweaks;
            setServoSliders('lf', t.left_front);
            setServoSliders('lm', t.left_middle);
            setServoSliders('lb', t.left_back);
            setServoSliders('rf', t.right_front);
            setServoSliders('rm', t.right_middle);
            setServoSliders('rb', t.right_back);
        }
    } catch (_) {
        // ignore
    }
}

async function applyServoTweaksLive() {
    const payload = collectServoTweaksPayload();
    try {
        if (window.__servoTweaksAbortController) {
            try { window.__servoTweaksAbortController.abort(); } catch (e) { }
        }
        window.__servoTweaksAbortController = new AbortController();
        await fetch(`${API_BASE}/servo_tweaks`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
            signal: window.__servoTweaksAbortController.signal,
        });
    } catch (_) {
        // ignore
    }

    // Always schedule a save to disk (debounced) so we persist tweaks.
    scheduleServoTweaksSave(payload);
}

function scheduleServoTweaksSave(payload) {
    if (window.__servoTweaksSaveTimer) {
        clearTimeout(window.__servoTweaksSaveTimer);
    }
    window.__servoTweaksSaveTimer = setTimeout(async () => {
        try {
            await fetch(`${API_BASE}/servo_tweaks/save`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload),
            });
        } catch (_) {
            // ignore
        }
    }, 250);
}

function resetServoTweaks() {
    setServoSliders('lf', [0.0, 0.0, 0.0]);
    setServoSliders('lm', [0.0, 0.0, 0.0]);
    setServoSliders('lb', [0.0, 0.0, 0.0]);
    setServoSliders('rf', [0.0, 0.0, 0.0]);
    setServoSliders('rm', [0.0, 0.0, 0.0]);
    setServoSliders('rb', [0.0, 0.0, 0.0]);
    applyServoTweaksLive();
}

async function saveServoTweaksNow() {
    const payload = collectServoTweaksPayload();
    try {
        await fetch(`${API_BASE}/servo_tweaks/save`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        });
    } catch (_) {
        // ignore
    }
}

// ===== AI CHAT =====

function initAIChat() {
    const sendBtn = document.getElementById('ai-chat-send');
    const chatInput = document.getElementById('ai-chat-input');

    if (!sendBtn || !chatInput) return;

    sendBtn.addEventListener('click', () => {
        const text = chatInput.value.trim();
        if (text.length > 0 && !aiChatSending) {
            sendAIChatMessage(text);
            chatInput.value = '';
        }
    });

    // Simulate Voice Button
    const voiceBtn = document.getElementById('ai-chat-voice');
    if (voiceBtn) {
        voiceBtn.addEventListener('click', async () => {
            if (aiChatSending) return;
            if (liveModeEnabled) {
                appendChatMessage('ai', '⚠️ Disable Live Mode to use manual recording.');
                return;
            }
            if (!voiceRecording) {
                await startVoiceRecording(voiceBtn);
            } else {
                stopVoiceRecording(voiceBtn);
            }
        });
    }

    const liveBtn = document.getElementById('ai-chat-live');
    if (liveBtn) {
        liveBtn.addEventListener('click', () => {
            toggleLiveMode(liveBtn);
        });
        updateLiveButton(liveBtn);
    }

    chatInput.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') {
            const text = chatInput.value.trim();
            if (text.length > 0 && !aiChatSending) {
                sendAIChatMessage(text);
                chatInput.value = '';
            }
        }
    });

    // Quick Actions
    const btnWalk = document.getElementById('ai-btn-walk');
    if (btnWalk) {
        btnWalk.addEventListener('click', () => {
            if (!aiChatSending) sendAIChatMessageWithOptions("walk forward", { speakReply: true });
        });
    }

    const btnStop = document.getElementById('ai-btn-stop');
    if (btnStop) {
        btnStop.addEventListener('click', () => {
            if (!aiChatSending) sendAIChatMessageWithOptions("halt", { speakReply: true });
        });
    }

    const btnSpin = document.getElementById('ai-btn-spin');
    if (btnSpin) {
        btnSpin.addEventListener('click', () => {
            if (!aiChatSending) sendAIChatMessageWithOptions("spin in place", { speakReply: true });
        });
    }

    // Poll AI health status
    checkAIHealth();
    setInterval(checkAIHealth, 5000);
}

async function startVoiceRecording(button) {
    try {
        voiceStream = await navigator.mediaDevices.getUserMedia({ audio: true });
        const preferredTypes = ['audio/mp3', 'audio/mpeg', 'audio/webm'];
        let options;
        for (const pt of preferredTypes) {
            if (MediaRecorder.isTypeSupported(pt)) {
                options = { mimeType: pt };
                break;
            }
        }
        voiceRecorder = new MediaRecorder(voiceStream, options);
        voiceChunks = [];

        voiceRecorder.ondataavailable = (event) => {
            if (event.data && event.data.size > 0) {
                voiceChunks.push(event.data);
            }
        };

        voiceRecorder.onstop = async () => {
            const blobType = (options && options.mimeType) ? options.mimeType : 'audio/webm';
            const audioBlob = new Blob(voiceChunks, { type: blobType });
            await sendVoiceCommand(audioBlob);
            if (voiceStream) {
                voiceStream.getTracks().forEach(track => track.stop());
                voiceStream = null;
            }
        };

        voiceRecorder.start();
        voiceRecording = true;
        button.classList.add('recording');
        button.textContent = '⏹️';
    } catch (error) {
        console.error('Microphone error:', error);
        appendChatMessage('ai', '⚠️ Microphone access denied or unavailable.');
    }
}

function stopVoiceRecording(button) {
    if (!voiceRecorder) return;
    voiceRecorder.stop();
    voiceRecording = false;
    button.classList.remove('recording');
    button.textContent = '🎙️';
}

async function sendVoiceCommand(audioBlob) {
    aiChatSending = true;
    const sendBtn = document.getElementById('ai-chat-send');
    sendBtn.disabled = true;

    const messagesDiv = document.getElementById('chat-messages');
    const thinkingEl = document.createElement('div');
    thinkingEl.className = 'chat-message ai-message';
    thinkingEl.id = 'chat-thinking';
    thinkingEl.innerHTML = '<div class="chat-thinking"><span class="dot"></span><span class="dot"></span><span class="dot"></span></div>';
    messagesDiv.appendChild(thinkingEl);
    messagesDiv.scrollTop = messagesDiv.scrollHeight;

    try {
        const response = await fetch(`${AI_API_BASE}/voice`, {
            method: 'POST',
            headers: { 'Content-Type': audioBlob.type || 'audio/webm' },
            body: audioBlob
        });

        const thinking = document.getElementById('chat-thinking');
        if (thinking) thinking.remove();

        if (!response.ok) {
            const err = await response.json().catch(() => ({}));
            appendChatMessage('ai', `⚠️ Voice error: ${err.detail || 'Unknown error'}`);
        } else {
            const data = await response.json();
            if (data.transcript) {
                appendChatMessage('user', data.transcript);
            }
            if (data.reply) {
                appendChatMessage('ai', data.reply, data.actions || []);
            }
        }
    } catch (error) {
        const thinking = document.getElementById('chat-thinking');
        if (thinking) thinking.remove();
        console.error('Voice error:', error);
        appendChatMessage('ai', '⚠️ Could not reach AI voice service.');
    }

    aiChatSending = false;
    sendBtn.disabled = false;
    document.getElementById('ai-chat-input').focus();
}

async function checkAIHealth() {
    const dot = document.getElementById('ai-status-dot');
    const text = document.getElementById('ai-status-text');
    if (!dot || !text) return;

    try {
        const res = await fetch(`${AI_API_BASE}/health`, { signal: AbortSignal.timeout(2000) });
        if (res.ok) {
            const data = await res.json();
            dot.classList.add('connected');
            text.textContent = `AI Connected (${data.model || 'GPT'})`;
        } else {
            dot.classList.remove('connected');
            text.textContent = 'AI Disconnected';
        }
    } catch (_) {
        dot.classList.remove('connected');
        text.textContent = 'AI Disconnected';
    }
}

async function sendAIChatMessage(message) {
    return sendAIChatMessageWithOptions(message, {});
}

async function sendAIChatMessageWithOptions(message, options) {
    aiChatSending = true;
    const sendBtn = document.getElementById('ai-chat-send');
    sendBtn.disabled = true;

    // Add user message
    if (!options.skipUserAppend) {
        appendChatMessage('user', options.displayText || message);
    }

    // Add thinking indicator
    const messagesDiv = document.getElementById('chat-messages');
    const thinkingEl = document.createElement('div');
    thinkingEl.className = 'chat-message ai-message';
    thinkingEl.id = 'chat-thinking';
    thinkingEl.innerHTML = '<div class="chat-thinking"><span class="dot"></span><span class="dot"></span><span class="dot"></span></div>';
    messagesDiv.appendChild(thinkingEl);
    messagesDiv.scrollTop = messagesDiv.scrollHeight;

    try {
        const response = await fetch(`${AI_API_BASE}/chat`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ message: message })
        });

        // Remove thinking indicator
        const thinking = document.getElementById('chat-thinking');
        if (thinking) thinking.remove();

        if (response.ok) {
            const data = await response.json();
            const replyText = data.reply || 'Done.';
            appendChatMessage('ai', replyText, data.actions || []);

            // Speak the reply out loud if requested
            if (options.speakReply && data.reply) {
                speakText(data.reply);
            }
        } else {
            const err = await response.json().catch(() => ({}));
            appendChatMessage('ai', `⚠️ Error: ${err.error || 'Unknown error'}`);
        }
    } catch (error) {
        // Remove thinking indicator
        const thinking = document.getElementById('chat-thinking');
        if (thinking) thinking.remove();

        console.error('AI Chat error:', error);
        appendChatMessage('ai', '⚠️ Could not reach AI service. Is the Python AI module running?');
    }

    aiChatSending = false;
    sendBtn.disabled = false;
    document.getElementById('ai-chat-input').focus();
}

function updateLiveButton(button) {
    if (!button) return;
    if (liveModeEnabled) {
        button.classList.add('active');
        button.textContent = 'Live: On';
    } else {
        button.classList.remove('active');
        button.textContent = 'Live Mode';
    }
}

function toggleLiveMode(button) {
    if (liveModeEnabled) {
        stopLiveMode();
    } else {
        startLiveMode();
    }
    updateLiveButton(button);
}

function startLiveMode() {
    startLiveRecording();
}

function stopLiveMode() {
    stopLiveRecording();
}

function extractWakeCommand(text) {
    if (!text) return '';
    const lower = text.toLowerCase();
    const wakeWords = ['hexapod', 'ninja'];
    for (const wake of wakeWords) {
        const idx = lower.indexOf(wake);
        if (idx !== -1) {
            const after = text.slice(idx + wake.length).replace(/^\s*[:,]?\s*/g, '').trim();
            if (after.length > 0) return after;
        }
    }
    return '';
}

async function startLiveRecording() {
    if (liveRecording) return;
    try {
        liveStream = await navigator.mediaDevices.getUserMedia({
            audio: {
                channelCount: 1
            }
        });

        liveAudioContext = new (window.AudioContext || window.webkitAudioContext)();
        const source = liveAudioContext.createMediaStreamSource(liveStream);
        liveScriptProcessor = liveAudioContext.createScriptProcessor(512, 1, 1);

        const loc = window.location;
        const wsProtocol = loc.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${wsProtocol}//${loc.hostname}:3001/api/ai/wake`;
        liveWs = new WebSocket(wsUrl);

        liveWs.onopen = () => {
            console.log('Wake-word WS connected');
            source.connect(liveScriptProcessor);
            liveScriptProcessor.connect(liveAudioContext.destination);
        };

        liveWs.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                if (data.event === 'wake') {
                    appendChatMessage('ai', `👂 Wake word detected (${data.keyword})`);
                }
                if (data.transcript) {
                    appendChatMessage('user', data.transcript);
                }
                if (data.reply) {
                    appendChatMessage('ai', data.reply, data.actions || []);
                }
            } catch (e) {
                console.error('Wake WS parse error', e);
            }
        };

        liveWs.onerror = (e) => {
            console.error('Wake WS error', e);
        };

        liveWs.onclose = (event) => {
            console.log('Wake WS closed', event.code, event.reason);
            if (event.code === 1008 || event.code === 1011) {
                appendChatMessage('ai', `⚠️ Wake WS closed: ${event.reason || 'server error'}`);
                stopLiveRecording();
                return;
            }
            if (liveModeEnabled) {
                scheduleLiveReconnect();
            }
        };

        liveScriptProcessor.onaudioprocess = (event) => {
            if (!liveWs || liveWs.readyState !== WebSocket.OPEN) return;

            const inputData = event.inputBuffer.getChannelData(0);
            const downsampled = downsampleFloat32(inputData, liveAudioContext.sampleRate, LIVE_TARGET_SAMPLE_RATE);
            if (!downsampled || downsampled.length === 0) return;

            const pcm16 = new Int16Array(downsampled.length);
            for (let i = 0; i < downsampled.length; i++) {
                const s = Math.max(-1, Math.min(1, downsampled[i]));
                pcm16[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
            }
            const uint8 = new Uint8Array(pcm16.buffer);
            let binary = '';
            for (let i = 0; i < uint8.byteLength; i++) {
                binary += String.fromCharCode(uint8[i]);
            }
            liveWs.send(btoa(binary));
        };

        liveModeEnabled = true;
        liveRecording = true;
        liveReconnectDelay = 500;
    } catch (error) {
        console.error('Live mode mic error:', error);
        appendChatMessage('ai', '⚠️ Microphone access denied or unavailable.');
        liveModeEnabled = false;
        liveRecording = false;
    }
}

function stopLiveRecording() {
    liveModeEnabled = false;
    liveRecording = false;

    if (liveReconnectTimer) {
        clearTimeout(liveReconnectTimer);
        liveReconnectTimer = null;
    }
    if (liveScriptProcessor) {
        liveScriptProcessor.disconnect();
        liveScriptProcessor = null;
    }
    if (liveAudioContext) {
        liveAudioContext.close();
        liveAudioContext = null;
    }
    if (liveWs) {
        liveWs.close();
        liveWs = null;
    }
    if (liveStream) {
        liveStream.getTracks().forEach(track => track.stop());
        liveStream = null;
    }
}

function scheduleLiveReconnect() {
    if (liveReconnectTimer) return;
    liveReconnectTimer = setTimeout(() => {
        liveReconnectTimer = null;
        if (liveModeEnabled) {
            stopLiveRecording();
            startLiveRecording();
            liveReconnectDelay = Math.min(liveReconnectDelay * 2, 5000);
        }
    }, liveReconnectDelay);
}

function downsampleFloat32(buffer, inputRate, targetRate) {
    if (inputRate === targetRate) return buffer;
    if (inputRate < targetRate) return buffer;

    const ratio = inputRate / targetRate;
    const newLength = Math.round(buffer.length / ratio);
    const result = new Float32Array(newLength);
    let offsetResult = 0;
    let offsetBuffer = 0;

    while (offsetResult < result.length) {
        const nextOffsetBuffer = Math.round((offsetResult + 1) * ratio);
        let accum = 0;
        let count = 0;
        for (let i = offsetBuffer; i < nextOffsetBuffer && i < buffer.length; i++) {
            accum += buffer[i];
            count++;
        }
        result[offsetResult] = count > 0 ? accum / count : 0;
        offsetResult++;
        offsetBuffer = nextOffsetBuffer;
    }
    return result;
}

async function sendVoiceChunk(audioBlob) {
    if (!audioBlob || audioBlob.size < 2048) {
        return;
    }
    try {
        const response = await fetch(`${AI_API_BASE}/voice`, {
            method: 'POST',
            headers: { 'Content-Type': audioBlob.type || 'audio/webm' },
            body: audioBlob
        });

        if (!response.ok) {
            return;
        }
        const data = await response.json();
        if (!data.accepted) {
            return;
        }
        if (data.transcript) {
            appendChatMessage('user', data.transcript);
        }
        if (data.reply) {
            appendChatMessage('ai', data.reply, data.actions || []);
        }
    } catch (error) {
        console.error('Live chunk error:', error);
    }
}

function appendChatMessage(role, text, actions) {
    const messagesDiv = document.getElementById('chat-messages');
    if (!messagesDiv) return;

    const msgEl = document.createElement('div');
    msgEl.className = `chat-message ${role === 'user' ? 'user-message' : 'ai-message'}`;

    const bubbleEl = document.createElement('div');
    bubbleEl.className = 'chat-bubble';
    bubbleEl.textContent = text;
    msgEl.appendChild(bubbleEl);

    // Show action badges for AI messages
    if (role === 'ai' && actions && actions.length > 0) {
        const actionsEl = document.createElement('div');
        actionsEl.className = 'chat-actions';
        actions.forEach(a => {
            const badge = document.createElement('span');
            badge.className = 'chat-action-badge';
            badge.textContent = `⚡ ${a.function}${a.result ? ': ' + a.result : ''}`;
            actionsEl.appendChild(badge);
        });
        msgEl.appendChild(actionsEl);
    }

    messagesDiv.appendChild(msgEl);
    messagesDiv.scrollTop = messagesDiv.scrollHeight;
}
