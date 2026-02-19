import openai
import json
import logging
from config import settings
from .tools import available_tools

class LLMClient:
    def __init__(self):
        self.api_key = settings.OPENAI_API_KEY
        self.model = settings.OPENAI_MODEL
        if self.api_key:
            self.client = openai.OpenAI(api_key=self.api_key)
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
Context:
- Indoor/Outdoor environment.
- Safety is paramount.
- "Explore" means finding frontiers.
- "Find exit" means looking for gaps/doors.
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
                tool_choice="auto"
            )
            
            message = completion.choices[0].message
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
