from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import asyncio
import logging
from agent import Agent
from config import settings

# Logging setup
logging.basicConfig(level=logging.INFO)

app = FastAPI()

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

agent = Agent()

class ChatRequest(BaseModel):
    message: str

@app.on_event("startup")
async def startup_event():
    # Start the agent loop in background
    asyncio.create_task(agent.run_loop())

@app.get("/api/ai/health")
async def health_check():
    return agent.get_health()

@app.post("/api/ai/chat")
async def chat(request: ChatRequest):
    try:
        response = agent.process_command(request.message)
        return response
    except Exception as e:
        logging.error(f"Chat error: {e}")
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    # uvicorn handling here is usually for dev; 
    # production runs this file via `uvicorn main:app` or similar
    uvicorn.run(app, host=settings.AI_HOST, port=settings.AI_PORT)
