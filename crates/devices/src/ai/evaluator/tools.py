available_tools = [
    {
        "type": "function",
        "function": {
            "name": "explore",
            "description": "Start autonomous exploration of the environment to build a map.",
            "parameters": {
                "type": "object",
                "properties": {},
            },
        }
    },
    {
        "type": "function",
        "function": {
            "name": "stop",
            "description": "Immediately stop all movement and cancel current task.",
            "parameters": {
                "type": "object",
                "properties": {},
            },
        }
    },
    {
        "type": "function",
        "function": {
            "name": "find_exit",
            "description": "Search for an exit (door or corridor) and navigate through it.",
            "parameters": {
                "type": "object",
                "properties": {},
            },
        }
    },
    {
        "type": "function",
        "function": {
            "name": "goto_pose",
            "description": "Navigate to a specific coordinate relative to start.",
            "parameters": {
                "type": "object",
                "properties": {
                    "x": {"type": "number", "description": "X coordinate in meters"},
                    "y": {"type": "number", "description": "Y coordinate in meters"},
                },
                "required": ["x", "y"]
            },
        }
    },
    {
        "type": "function",
        "function": {
            "name": "speak",
            "description": "Say something using TTS.",
            "parameters": {
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to speak"},
                },
                "required": ["text"]
            },
        }
    }
]
