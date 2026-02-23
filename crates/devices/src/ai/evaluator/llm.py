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
            
            if message.tool_calls:
                # Return list of tool calls to be executed
                actions = []
                for tool_call in message.tool_calls:
                    actions.append({
                        "id": tool_call.id,
                        "function": tool_call.function.name,
                        "arguments": json.loads(tool_call.function.arguments)
                    })
                return {"actions": actions, "reply": message.content}
            else:
                return {"reply": message.content}
                
        except Exception as e:
            logging.error(f"LLM Error: {e}")
            return {"error": str(e)}
