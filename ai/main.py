from fastapi import FastAPI, HTTPException, Request, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import asyncio
import logging
import os
import re
import tempfile
import base64
import io
import math
import wave
from array import array
from collections import deque
from contextlib import asynccontextmanager
import openai
import json
import websockets
import numpy as np
from agent import Agent
from config import settings

# Logging setup
logging.basicConfig(level=logging.INFO)

agent = Agent()
stt_client = None

def _normalize_mic_source(value: str) -> str:
    if not value:
        return "web"
    source = value.strip().lower()
    if source == "onboard":
        logging.warning("Onboard mic selected but not implemented; using web panel mic.")
        return "web"
    if source != "web":
        logging.warning(f"Unknown mic source '{value}', using web panel mic.")
        return "web"
    return "web"

MIC_SOURCE = _normalize_mic_source(settings.MIC_SOURCE)

def _parse_csv(value: str) -> list[str]:
    if not value:
        return []
    return [item.strip() for item in value.split(",") if item.strip()]

def _looks_like_path(value: str) -> bool:
    if not value:
        return False
    if value.endswith(".ppn"):
        return True
    if value.startswith(("~", ".")):
        return True
    if os.path.isabs(value):
        return True
    return "/" in value or "\\" in value

def _expand_keyword_path(value: str) -> str:
    return os.path.abspath(os.path.expanduser(os.path.expandvars(value)))

def _split_keywords_and_paths(
    keyword_paths_raw: list[str],
    keywords_raw: list[str],
) -> tuple[list[str], list[str], list[str]]:
    keyword_paths: list[str] = []
    keywords: list[str] = []
    errors: list[str] = []

    for path in keyword_paths_raw:
        expanded = _expand_keyword_path(path)
        if not os.path.isfile(expanded):
            errors.append(f"Keyword path not found: {expanded}")
            continue
        keyword_paths.append(expanded)

    for entry in keywords_raw:
        if _looks_like_path(entry) or entry.endswith(".ppn"):
            expanded = _expand_keyword_path(entry)
            if not os.path.isfile(expanded):
                errors.append(f"Keyword path not found: {expanded}")
                continue
            keyword_paths.append(expanded)
        else:
            keywords.append(entry)

    return keyword_paths, keywords, errors

def _normalize_pcm16(samples: array, target_rms: float = 4000.0) -> array:
    if not samples:
        return samples
    acc = 0.0
    for s in samples:
        acc += s * s
    rms = math.sqrt(acc / len(samples)) if samples else 0.0
    if rms <= 1.0:
        return samples
    gain = target_rms / rms
    if gain <= 0:
        return samples
    out = array("h")
    for s in samples:
        v = int(max(-32768, min(32767, s * gain)))
        out.append(v)
    return out

def _pcm16_to_wav_bytes(samples: array, sample_rate: int) -> bytes:
    samples = _normalize_pcm16(samples)
    with io.BytesIO() as buf:
        with wave.open(buf, "wb") as wf:
            wf.setnchannels(1)
            wf.setsampwidth(2)
            wf.setframerate(sample_rate)
            wf.writeframes(samples.tobytes())
        return buf.getvalue()

def _is_transcript_good(text: str) -> bool:
    if not text:
        return False
    stripped = text.strip()
    if len(stripped) < settings.OPENAI_TRANSCRIBE_MIN_CHARS:
        return False
    alpha = sum(1 for c in stripped if c.isalpha())
    ratio = alpha / max(1, len(stripped))
    return ratio >= settings.OPENAI_TRANSCRIBE_MIN_ALPHA_RATIO

def _transcribe_with_model(file_path: str, model: str) -> str:
    with open(file_path, "rb") as audio_file:
        transcript = stt_client.audio.transcriptions.create(
            model=model,
            file=audio_file,
            response_format="text",
            language=settings.OPENAI_TRANSCRIBE_LANGUAGE,
            temperature=0.0,
            prompt=settings.OPENAI_TRANSCRIBE_PROMPT,
        )
    if isinstance(transcript, str):
        return transcript.strip()
    return str(transcript).strip()

def _save_transcribe_audio(data: bytes, suffix: str) -> None:
    if not settings.SAVE_TRANSCRIBE_AUDIO:
        return
    try:
        os.makedirs(settings.TRANSCRIBE_AUDIO_DIR, exist_ok=True)
        timestamp = int(asyncio.get_event_loop().time() * 1000)
        filename = f"audio_{timestamp}{suffix}"
        out_path = os.path.join(settings.TRANSCRIBE_AUDIO_DIR, filename)
        with open(out_path, "wb") as f:
            f.write(data)
    except Exception as e:
        logging.warning(f"Failed to save transcribe audio: {e}")

if settings.OPENAI_API_KEY:
    stt_client = openai.OpenAI(api_key=settings.OPENAI_API_KEY, max_retries=0)
else:
    logging.warning("OpenAI API Key not found. Whisper disabled.")

@asynccontextmanager
async def lifespan(app: FastAPI):
    # Startup: Start the agent loop
    loop_task = asyncio.create_task(agent.run_loop())
    yield
    # Shutdown logic
    loop_task.cancel()
    try:
        await loop_task
    except asyncio.CancelledError:
        pass

app = FastAPI(lifespan=lifespan)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

class ChatRequest(BaseModel):
    message: str


class Waypoint(BaseModel):
    x: float
    y: float


class NavigateRequest(BaseModel):
    waypoints: list[Waypoint]
    mode: str = "replace"


def _extract_wake_command(text: str) -> tuple[bool, str]:
    if not text:
        return False, ""
    cleaned = text.strip()
    match = re.search(r"\b(hexapod|ninja)\b\s*[:,]?\s*(.+)$", cleaned, re.IGNORECASE)
    if not match:
        return False, ""
    command = match.group(2).strip()
    if not command:
        return False, ""
    return True, command


async def _transcribe_audio(data: bytes, suffix: str) -> str:
    if not stt_client:
        raise HTTPException(status_code=503, detail="Whisper not configured")

    if not data:
        raise HTTPException(status_code=400, detail="Empty audio upload")

    _save_transcribe_audio(data, suffix)

    with tempfile.NamedTemporaryFile(delete=False, suffix=suffix) as tmp:
        tmp.write(data)
        tmp_path = tmp.name

    try:
        if "gpt" in settings.OPENAI_WHISPER_MODEL and "audio" in settings.OPENAI_WHISPER_MODEL and suffix in [".mp3", ".wav"]:
            fmt = suffix.replace(".", "").lower()
            b64_data = base64.b64encode(data).decode('utf-8')
            resp = stt_client.chat.completions.create(
                model=settings.OPENAI_WHISPER_MODEL,
                modalities=["text"],
                messages=[
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": (
                                "Transcribe the following audio precisely in English. "
                                "Only output the transcription, nothing else. "
                                "If you hear nothing, output nothing. "
                                f"Wake words: {settings.OPENAI_TRANSCRIBE_PROMPT}."
                            )},
                            {"type": "input_audio", "input_audio": {"data": b64_data, "format": fmt}}
                        ]
                    }
                ],
                temperature=0.0
            )
            transcript_text = (resp.choices[0].message.content or "").strip()
        else:
            model_to_use = "whisper-1" if "gpt" in settings.OPENAI_WHISPER_MODEL else settings.OPENAI_WHISPER_MODEL
            transcript_text = _transcribe_with_model(tmp_path, model_to_use)

        if _is_transcript_good(transcript_text):
            return transcript_text

        retry_model = settings.OPENAI_TRANSCRIBE_RETRY_MODEL.strip()
        if retry_model and retry_model != settings.OPENAI_WHISPER_MODEL:
            retry_text = _transcribe_with_model(tmp_path, retry_model)
            if _is_transcript_good(retry_text):
                return retry_text

        return transcript_text
    finally:
        try:
            os.remove(tmp_path)
        except OSError:
            pass

@app.get("/api/ai/health")
async def health_check():
    payload = agent.get_health()
    payload["mic_source"] = MIC_SOURCE
    return payload

@app.post("/api/ai/chat")
async def chat(request: ChatRequest):
    try:
        response = await agent.process_command(request.message)
        return response
    except Exception as e:
        logging.error(f"Chat error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/api/ai/navigation")
async def get_navigation():
    return agent.get_navigation_status()


@app.post("/api/ai/navigation")
async def set_navigation(request: NavigateRequest):
    waypoints = [(wp.x, wp.y) for wp in request.waypoints]
    mode = request.mode.lower().strip() if request.mode else "replace"
    if mode not in ("replace", "append"):
        raise HTTPException(status_code=400, detail="mode must be 'replace' or 'append'")
    agent.set_navigation_waypoints(waypoints, mode=mode)
    return agent.get_navigation_status()


@app.post("/api/ai/navigation/clear")
async def clear_navigation():
    agent.clear_navigation_waypoints()
    return agent.get_navigation_status()


@app.post("/api/ai/voice")
async def voice_command(request: Request):
    try:
        data = await request.body()
        content_type = request.headers.get("content-type", "").lower().split(";")[0].strip()
        suffix = ".webm"
        if "audio/wav" in content_type or "audio/x-wav" in content_type:
            suffix = ".wav"
        elif "audio/mpeg" in content_type or "audio/mp3" in content_type:
            suffix = ".mp3"
        elif "audio/ogg" in content_type:
            suffix = ".ogg"

        transcript = await _transcribe_audio(data, suffix)
        logging.info(f"Manual Audio Transcribed Text: '{transcript}'")
        accepted, command = _extract_wake_command(transcript)
        payload = {"transcript": transcript, "accepted": accepted, "command": command}

        if accepted and command:
            response = await agent.process_command(command)
            reply = response.get("reply") or ""
            actions = response.get("actions", [])
            payload.update({"reply": reply, "actions": actions})
            if reply:
                agent.client.speak(reply)
        else:
            payload.update({"reply": "Say 'hexapod' or 'ninja' followed by a command."})

        return payload
    except HTTPException:
        raise
    except Exception as e:
        logging.error(f"Voice error: {e}")
        raise HTTPException(status_code=500, detail=str(e))

@app.websocket("/api/ai/realtime")
async def websocket_realtime(websocket: WebSocket):
    await websocket.accept()
    if not settings.OPENAI_API_KEY:
        await websocket.close(code=1008, reason="OpenAI API Key not missing")
        return

    url = "wss://api.openai.com/v1/realtime?model=gpt-4o-realtime-preview-2024-10-01"
    headers = {
        "Authorization": f"Bearer {settings.OPENAI_API_KEY}",
        "OpenAI-Beta": "realtime=v1"
    }

    try:
        async with websockets.connect(url, additional_headers=headers) as openai_ws:
            # Configure OpenAI session to require audio input and return text (and/or audio)
            await openai_ws.send(json.dumps({
                "type": "session.update",
                "session": {
                    "modalities": ["text"],
                    "instructions": "Listen for the wake words 'hexapod' or 'ninja' followed by a command. If heard, transcribe it precisely. Otherwise ignore.",
                    "turn_detection": {"type": "server_vad"},
                }
            }))

            async def rx_from_browser():
                try:
                    while True:
                        msg = await websocket.receive_text()
                        await openai_ws.send(json.dumps({
                            "type": "input_audio_buffer.append",
                            "audio": msg
                        }))
                except WebSocketDisconnect:
                    pass
                except Exception as e:
                    logging.info(f"Browser disconnect: {e}")

            async def rx_from_openai():
                try:
                    while True:
                        resp_str = await openai_ws.recv()
                        resp = json.loads(resp_str)
                        logging.info(f"OpenAI WS Event: {resp.get('type')}")
                        if resp.get("type") == "response.audio_transcript.done":
                            text = resp.get("transcript", "")
                            if text:
                                logging.info(f"Realtime Audio Transcribed Text: '{text}'")
                                accepted, command = _extract_wake_command(text)
                                payload = {"transcript": text, "accepted": accepted, "command": command}
                                if accepted and command:
                                    # Process command in background
                                    res = await agent.process_command(command)
                                    reply = res.get("reply") or ""
                                    actions = res.get("actions", [])
                                    payload.update({"reply": reply, "actions": actions})
                                    if reply:
                                        agent.client.speak(reply)
                                    await websocket.send_json(payload)
                except Exception as e:
                    logging.info(f"OpenAI WS disconnect: {e}")

            await asyncio.gather(rx_from_browser(), rx_from_openai())

    except Exception as e:
        logging.error(f"Realtime WS error: {e}")
        try:
            await websocket.close(code=1011)
        except:
            pass

@app.websocket("/api/ai/wake")
async def websocket_wake(websocket: WebSocket):
    await websocket.accept()

    if not settings.PICOVOICE_ACCESS_KEY:
        await websocket.close(code=1008, reason="Picovoice access key missing")
        return

    try:
        import pvporcupine
    except Exception:
        await websocket.close(code=1011, reason="Porcupine not installed")
        return

    keyword_paths_raw = _parse_csv(settings.PORCUPINE_KEYWORD_PATHS)
    keywords_raw = _parse_csv(settings.PORCUPINE_KEYWORDS)
    keyword_paths, keywords, path_errors = _split_keywords_and_paths(
        keyword_paths_raw,
        keywords_raw,
    )
    if path_errors:
        logging.error("; ".join(path_errors))
        await websocket.close(code=1008, reason=path_errors[0])
        return
    if not keyword_paths and not keywords:
        await websocket.close(code=1008, reason="Porcupine keywords not configured")
        return

    sensitivities_raw = _parse_csv(settings.PORCUPINE_SENSITIVITIES)
    if keyword_paths and keywords:
        try:
            for keyword in keywords:
                if keyword in pvporcupine.KEYWORD_PATHS:
                    keyword_paths.append(pvporcupine.KEYWORD_PATHS[keyword])
                else:
                    raise ValueError(f"Unknown Porcupine keyword: {keyword}")
            keywords = []
        except Exception as e:
            logging.error(f"Porcupine keyword error: {e}")
            await websocket.close(code=1008, reason="Porcupine keyword error")
            return

    if keyword_paths:
        keyword_count = len(keyword_paths)
    else:
        keyword_count = len(keywords)
    if len(sensitivities_raw) != keyword_count:
        sensitivities = [0.65] * keyword_count
    else:
        sensitivities = [float(s) for s in sensitivities_raw]

    try:
        porcupine = pvporcupine.create(
            access_key=settings.PICOVOICE_ACCESS_KEY,
            keyword_paths=keyword_paths or None,
            keywords=keywords or None,
            sensitivities=sensitivities,
        )
    except Exception as e:
        logging.error(f"Porcupine init failed: {e}")
        await websocket.close(code=1011, reason="Porcupine init failed")
        return

    sample_rate = porcupine.sample_rate
    frame_length = porcupine.frame_length

    pre_roll_samples = int(sample_rate * settings.WAKE_PRE_ROLL_MS / 1000)
    trim_samples = int(sample_rate * settings.WAKE_TRIM_MS / 1000)
    silence_samples = int(sample_rate * settings.WAKE_SILENCE_MS / 1000)
    max_samples = int(sample_rate * settings.WAKE_MAX_UTTERANCE_MS / 1000)
    min_samples = int(sample_rate * settings.WAKE_MIN_UTTERANCE_MS / 1000)

    pcm_buffer = array("h")
    pre_roll = deque(maxlen=pre_roll_samples)
    listening = False
    utterance = array("h")
    silence_run = 0
    skip_after_wake = 0

    try:
        while True:
            msg = await websocket.receive()
            if "text" in msg and msg["text"]:
                raw = base64.b64decode(msg["text"])
            elif "bytes" in msg and msg["bytes"]:
                raw = msg["bytes"]
            else:
                continue

            if len(raw) % 2 != 0:
                raw = raw[:-1]
            if not raw:
                continue

            pcm = np.frombuffer(raw, dtype=np.int16)
            pcm_buffer.extend(pcm.tolist())

            while len(pcm_buffer) >= frame_length:
                frame = pcm_buffer[:frame_length]
                del pcm_buffer[:frame_length]

                rms = 0.0
                if frame_length > 0:
                    rms = math.sqrt(sum(s * s for s in frame) / frame_length)

                if not listening:
                    pre_roll.extend(frame)
                    try:
                        keyword_index = porcupine.process(frame)
                    except Exception as e:
                        logging.warning(f"Porcupine process error: {e}")
                        keyword_index = -1
                    if keyword_index >= 0:
                        listening = True
                        utterance = array("h")
                        silence_run = 0
                        skip_after_wake = trim_samples
                        keyword_name = (
                            keywords[keyword_index]
                            if keywords
                            else os.path.basename(keyword_paths[keyword_index])
                        )
                        await websocket.send_json({"event": "wake", "keyword": keyword_name})
                else:
                    if skip_after_wake > 0:
                        skip_after_wake = max(0, skip_after_wake - frame_length)
                    else:
                        utterance.extend(frame)
                    if rms < settings.WAKE_RMS_THRESHOLD:
                        silence_run += frame_length
                    else:
                        silence_run = 0

                    if silence_run >= silence_samples or len(utterance) >= max_samples:
                        if len(utterance) >= min_samples:
                            wav_bytes = _pcm16_to_wav_bytes(utterance, sample_rate)
                            transcript = await _transcribe_audio(wav_bytes, ".wav")
                            accepted, command = _extract_wake_command(transcript)
                            if not command:
                                command = transcript.strip()
                            payload = {
                                "transcript": transcript,
                                "accepted": bool(command),
                                "command": command,
                            }
                            if command:
                                response = await agent.process_command(command)
                                reply = response.get("reply") or ""
                                actions = response.get("actions", [])
                                payload.update({"reply": reply, "actions": actions})
                                if reply:
                                    agent.client.speak(reply)
                            await websocket.send_json(payload)

                        listening = False
                        utterance = array("h")
                        silence_run = 0
                        skip_after_wake = 0
                        pre_roll.clear()

    except WebSocketDisconnect:
        pass
    except Exception as e:
        logging.error(f"Wake WS error: {e}")
    finally:
        porcupine.delete()

if __name__ == "__main__":
    import uvicorn
    # uvicorn handling here is usually for dev; 
    # production runs this file via `uvicorn main:app` or similar
    uvicorn.run(app, host=settings.AI_HOST, port=settings.AI_PORT)
