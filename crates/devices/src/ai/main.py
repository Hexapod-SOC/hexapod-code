from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import asyncio
import logging
from contextlib import asynccontextmanager
from agent import Agent
from config import settings

# Logging setup
logging.basicConfig(level=logging.INFO)

agent = Agent()

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

if __name__ == "__main__":
    import uvicorn
    # uvicorn handling here is usually for dev; 
    # production runs this file via `uvicorn main:app` or similar
    uvicorn.run(app, host=settings.AI_HOST, port=settings.AI_PORT)
