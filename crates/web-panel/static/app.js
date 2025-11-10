// API Configuration - Use current window location for API calls
const API_BASE = `${window.location.protocol}//${window.location.hostname}:3000/api`;

// State
let currentGait = 'tri';
let isDragging = false;
let currentJoystick = null;
let gamepadConnected = false;
let gamepadLayout = 'xbox'; // 'xbox' or 'playstation'
let gamepadIndex = null;
let gamepadAnimationFrame = null;

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    initJoysticks();
    initGaitSelector();
    initPoseControls();
    initEmergencyStop();
    initTTS();
    initGamepad();
    initCustomGaitControls();
    startStatusUpdates();
});

// Joystick Control
function initJoysticks() {
    const moveJoystick = document.getElementById('move-joystick');
    const rotateJoystick = document.getElementById('rotate-joystick');

    setupJoystick(moveJoystick, (x, y) => {
        // x = strafe (left/right), y = forward (forward/back)
        sendMoveCommand({ forward: y * 100, strafe: x * 100, rotation: 0.0 });
    });

    setupJoystick(rotateJoystick, (x, y) => {
        // x = rotation
        sendMoveCommand({ forward: 0.0, strafe: 0.0, rotation: x });
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
    const gaitBtns = document.querySelectorAll('.gait-btn');
    gaitBtns.forEach(btn => {
        btn.addEventListener('click', () => {
            gaitBtns.forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            currentGait = btn.dataset.gait;
            setGait(currentGait);
        });
    });

    // Set default active
    document.querySelector('[data-gait="tri"]').classList.add('active');
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

    let poseUpdateTimeout = null;

    Object.keys(sliders).forEach(key => {
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
}

// Emergency Stop
function initEmergencyStop() {
    const stopBtn = document.getElementById('emergency-stop');
    stopBtn.addEventListener('click', () => {
        emergencyStop();
    });
}

// Text-to-Speech
function initTTS() {
    const speakBtn = document.getElementById('speak-btn');
    const ttsInput = document.getElementById('tts-input');
    const ttsStatus = document.getElementById('tts-status');

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
            console.log(`Gait changed to: ${data.current_gait}`);
        }
    } catch (error) {
        console.error('Error setting gait:', error);
        updateConnectionStatus(false);
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
        const response = await fetch(`${API_BASE}/stop`, {
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
        'push-fraction','speed-mult','step-mult','lift-mult','max-step','max-speed',
        'off-lf','off-lm','off-lb','off-rf','off-rm','off-rb'
    ];
    ids.forEach(id => {
        const el = document.getElementById(id);
        const val = document.getElementById(id + '-val');
        if (el && val) {
            el.addEventListener('input', () => {
                val.textContent = el.value;
            });
        }
    });

    const applyBtn = document.getElementById('apply-custom-gait');
    if (!applyBtn) return;

    applyBtn.addEventListener('click', async () => {
        const payload = {
            name: document.getElementById('custom-gait-name').value || 'custom',
            leg_cycle_offsets: {
                left_front: parseFloat(document.getElementById('off-lf').value),
                left_middle: parseFloat(document.getElementById('off-lm').value),
                left_back: parseFloat(document.getElementById('off-lb').value),
                right_front: parseFloat(document.getElementById('off-rf').value),
                right_middle: parseFloat(document.getElementById('off-rm').value),
                right_back: parseFloat(document.getElementById('off-rb').value),
            },
            push_fraction: parseFloat(document.getElementById('push-fraction').value),
            speed_multiplier: parseFloat(document.getElementById('speed-mult').value),
            step_length_multiplier: parseFloat(document.getElementById('step-mult').value),
            lift_height_multiplier: parseFloat(document.getElementById('lift-mult').value),
            max_step_length: parseFloat(document.getElementById('max-step').value),
            max_speed: parseFloat(document.getElementById('max-speed').value)
        };

        try {
            const res = await fetch(`${API_BASE}/custom_gait`, {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify(payload)
            });
            if (res.ok) {
                const data = await res.json();
                console.log('Custom gait applied:', data.current_gait);
            } else {
                console.error('Failed to apply custom gait', res.status);
                updateConnectionStatus(false);
            }
        } catch (err) {
            console.error('Error applying custom gait', err);
            updateConnectionStatus(false);
        }
    });
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
            document.getElementById('gait-status').textContent = status.gait_name || 'unknown';
            document.getElementById('state-status').textContent = battery.power_state || 'unknown';
            
            // Update battery display
            document.getElementById('voltage-value').textContent = `${battery.voltage.toFixed(2)}V`;
            document.getElementById('current-value').textContent = `${battery.current.toFixed(2)}A`;

            updateConnectionStatus(true);
        } else {
            updateConnectionStatus(false);
        }
    } catch (error) {
        console.error('Error fetching status:', error);
        updateConnectionStatus(false);
    }
}

function updateConnectionStatus(connected) {
    const statusElement = document.getElementById('connection-status');
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

    const strafeX = processAxis(leftStickX);
    const forwardY = -processAxis(leftStickY); // Invert Y axis
    const rotation = processAxis(rightStickX);

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
