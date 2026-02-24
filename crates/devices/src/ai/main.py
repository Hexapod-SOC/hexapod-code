from fastapi import FastAPI, HTTPException, Request, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import asyncio
import logging
import os
import re
import tempfile
from contextlib import asynccontextmanager
import openai
import json
import websockets
from agent import Agent
from config import settings

# Logging setup
logging.basicConfig(level=logging.INFO)

agent = Agent()
stt_client = None

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


def _extract_wake_command(text: str) -> tuple[bool, str]:
    if not text:
        return False, ""
    match = re.match(r"^\s*(hexapod|ninja)\s*[:,]?\s*(.+)$", text.strip(), re.IGNORECASE)
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

    with tempfile.NamedTemporaryFile(delete=False, suffix=suffix) as tmp:
        tmp.write(data)
        tmp_path = tmp.name

    if "gpt" in settings.OPENAI_WHISPER_MODEL and "audio" in settings.OPENAI_WHISPER_MODEL and suffix in [".mp3", ".wav"]:
        import base64
        fmt = suffix.replace(".", "").lower()
        b64_data = base64.b64encode(data).decode('utf-8')
        try:
            resp = stt_client.chat.completions.create(
                model=settings.OPENAI_WHISPER_MODEL,
                modalities=["text"],
                messages=[
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "Transcribe the following audio precisely. Only output the transcription, nothing else. If you hear nothing, output nothing."},
                            {"type": "input_audio", "input_audio": {"data": b64_data, "format": fmt}}
                        ]
                    }
                ],
                temperature=0.0
            )
            return resp.choices[0].message.content.strip()
        finally:
            try:
                os.remove(tmp_path)
            except OSError:
                pass
    else:
        try:
            with open(tmp_path, "rb") as audio_file:
                # Fallback to standard whisper for formats like webm
                model_to_use = "whisper-1" if "gpt" in settings.OPENAI_WHISPER_MODEL else settings.OPENAI_WHISPER_MODEL
                transcript = stt_client.audio.transcriptions.create(
                    model=model_to_use,
                    file=audio_file,
                    response_format="text",
                    language="en",
                    temperature=0.0,
                    prompt="hexapod ninja",
                )
            if isinstance(transcript, str):
                return transcript.strip()
            return str(transcript).strip()
        finally:
            try:
                os.remove(tmp_path)
            except OSError:
                pass

@app.get("/api/ai/health")
async def health_check():
    return agent.get_health()

@app.post("/api/ai/chat")
async def chat(request: ChatRequest):
    try:
        response = await agent.process_command(request.message)
        return response
    except Exception as e:
        logging.error(f"Chat error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


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

if __name__ == "__main__":
    import uvicorn
    # uvicorn handling here is usually for dev; 
    # production runs this file via `uvicorn main:app` or similar
    uvicorn.run(app, host=settings.AI_HOST, port=settings.AI_PORT)
