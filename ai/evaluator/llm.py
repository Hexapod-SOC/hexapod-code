import openai
import json
import logging
import re
from config import settings
from .tools import available_tools

class LLMClient:
    def __init__(self):
        self.api_key = settings.OPENAI_API_KEY
        self.model = settings.OPENAI_MODEL
        if self.api_key:
            # max_retries=0: fail immediately so our timeout fires without 2+ retries first
            self.client = openai.OpenAI(api_key=self.api_key, max_retries=0)
        else:
            self.client = None
            logging.warning("OpenAI API Key not found. LLM disabled.")

    def parse_command(self, user_text: str, history: list = None) -> dict:
        """
        Convert user text into a structured plan using function calling.
        """
        if not self.client:
            return {"error": "LLM not configured"}

        messages = [
            {"role": "system", "content": """
You are the AI controller for a Hexapod robot.
Your goal is to interpret user commands and execute them using the available tools.
You can execute multiple tools in sequence.
If the request is ambiguous, ask for clarification (but prefer acting if reasonable default exists).
Keep replies short and direct (1-2 sentences). Avoid extra commentary.
The robot has a LiDAR, Camera, and can move.
YOU ARE IN CONTROL OF A PHYSICAL ROBOT.
Context:
- Indoor/Outdoor environment.
- Safety is paramount.
- "Explore" means finding frontiers. YOU MUST USE THE `explore` TOOL.
- "Find exit" means looking for gaps/doors.
- If the user says "walk", "keep going", "walk until I stop you" or similar open-ended movement: use the `walk` tool (no duration, runs until stopped).
- If the user specifies a time like "move forward 3 seconds": use the `move` tool with that duration.
- "Stop" or "halt" should call the `stop` tool.
- ALWAYS use tools to interact with the world. Do not just describe what you will do.
"""},
        ]
        
        if history:
            messages.extend(history)
            
        messages.append({"role": "user", "content": user_text})

        try:
            completion = self.client.chat.completions.create(
                model=self.model,
                messages=messages,
                tools=available_tools,
                tool_choice="auto",
                timeout=10,  # fail fast — don't hang the event loop thread
            )
            
            message = completion.choices[0].message
            logging.info(f"LLM Response: Content='{message.content}', ToolCalls={message.tool_calls}")
            
            reply_text = (message.content or "").strip()
            if reply_text:
                reply_text = self._shorten_reply(reply_text)

            if message.tool_calls:
                # Return list of tool calls to be executed
                actions = []
                for tool_call in message.tool_calls:
                    actions.append({
                        "id": tool_call.id,
                        "function": tool_call.function.name,
                        "arguments": json.loads(tool_call.function.arguments)
                    })
                return {"actions": actions, "reply": reply_text}
            else:
                return {"reply": reply_text}
                
        except Exception as e:
            logging.error(f"LLM Error: {e}")
            return {"error": str(e)}

    def _shorten_reply(self, text: str, max_chars: int = 200) -> str:
        if len(text) <= max_chars:
            return text
        sentences = re.split(r"(?<=[.!?])\s+", text)
        if sentences:
            short = sentences[0].strip()
            if short:
                return short[:max_chars].rstrip()
        return text[:max_chars].rstrip()
