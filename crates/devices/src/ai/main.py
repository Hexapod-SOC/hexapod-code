from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import asyncio
import logging
import os
import re
import tempfile
from contextlib import asynccontextmanager
import openai
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

    try:
        with open(tmp_path, "rb") as audio_file:
            transcript = stt_client.audio.transcriptions.create(
                model=settings.OPENAI_WHISPER_MODEL,
                file=audio_file,
                response_format="text",
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

if __name__ == "__main__":
    import uvicorn
    # uvicorn handling here is usually for dev; 
    # production runs this file via `uvicorn main:app` or similar
    uvicorn.run(app, host=settings.AI_HOST, port=settings.AI_PORT)
