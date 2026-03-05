# Hexapod AI Controller

This package implements the high-level AI control system for the Hexapod robot. It runs as a separate Python process alongside the main Rust firmware.

## Architecture

- **Agent (`agent.py`)**: The central brain that maintains state and executes the control loop.
- **Behaviors (`behaviors/`)**: Modular tasks like `Explore` and `FindExit`.
- **Navigation (`navigation/`)**: A* pathfinding on occupancy grids and a local motion controller with obstacle avoidance.
- **Sensors (`sensors/`)**: Adapters for LiDAR and Camera data.
- **LLM (`evaluator/`)**: OpenAI-powered command interpreter that converts natural language into structured plans.
- **API Server (`main.py`)**: FastAPI server for web panel communication.

## Setup

1.  **Install Dependencies**:
    The main hexapod `remoterun.ps1` script handles synchronization and installation.
    Manually:
    ```bash
    pip install -r requirements.txt
    ```

2.  **Configuration**:
    - Build/deploy the main Rust project. The Rust binary spawns this Python module automatically.
    - Ensure `apikey.txt` exists in this directory (or set `OPENAI_API_KEY` env var).

3.  **Running Manually (Dev)**:
    ```bash
    # Mocking the Rust API
    export HEXAPOD_API_BASE="http://localhost:3000/api"
    export AI_CHAT_PORT=3001
    python main.py
    ```

## Usage

The AI Controller exposes an HTTP API on port `3001` (by default).

-   **Chat**: `POST /api/ai/chat`
    -   Body: `{"message": "Explore the room"}`
    -   Response: `{"reply": "Starting exploration...", "actions": [...]}`

-   **Health**: `GET /api/ai/health`
    -   Response: `{"status": "running", "task": "explore", ...}`

-   **Navigation**: `POST /api/ai/navigation`
    -   Body: `{ "waypoints": [{"x": 1.2, "y": -0.4}], "mode": "replace" }`
    -   Response: `{ "task": "navigate", "waypoints": [...] }`

-   **Clear Navigation**: `POST /api/ai/navigation/clear`

-   **Camera Status**: `GET /api/ai/camera/status`
    -   Response: `{ "available": true, "device_id": 0, "width": 1280, "height": 720, "fps": 15 }`

-   **Camera Snapshot**: `GET /api/ai/camera/frame`
    -   Response: `image/jpeg`

-   **Camera Stream (MJPEG)**: `GET /api/ai/camera/stream`
    -   Response: `multipart/x-mixed-replace` MJPEG stream

-   **Camera Vision (Chat)**: `POST /api/ai/chat`
    -   Body: `{"message": "look"}` or `{"message": "what do you see?"}`

## Extending

-   **Add a new Tool**:
    1.  Define the function in `evaluator/tools.py`.
    2.  Implement the logic in `agent.py` under `process_command`.
-   **Add a new Behavior**:
    1.  Create `behaviors/my_behavior.py`.
    2.  Implement a `step()` method.
    3.  Integrate into `Agent.run_loop`.

## Camera Configuration

Set these environment variables to match the Pi camera:

- `AI_CAMERA_DEVICE_ID` (default `0`)
- `AI_CAMERA_WIDTH` (default `1280`)
- `AI_CAMERA_HEIGHT` (default `720`)
- `AI_CAMERA_FPS` (default `15`)
- `AI_CAMERA_STREAM_FPS` (default `10`)
- `AI_CAMERA_JPEG_QUALITY` (default `85`)

### Raspberry Pi (DietPi) setup notes

- If you see `cv2` missing `VideoCapture`, use the Picamera2 backend:
    - `sudo apt install -y python3-picamera2 python3-pil`
- To use OpenCV directly:
    - `sudo apt install -y python3-opencv`

## Vision-Assisted Find Exit

Enable camera help when choosing exit frontiers:

- `EXIT_USE_VISION` (default `true`)
- `EXIT_VISION_REFRESH_S` (default `8.0`)
- `EXIT_VISION_BONUS` (default `40.0`)
- `EXIT_VISION_PROMPT` (customize the direction query)
