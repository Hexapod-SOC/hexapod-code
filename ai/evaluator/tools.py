available_tools = [
    {
        "type": "function",
        "function": {
            "name": "walk",
            "description": "Walk the robot indefinitely in a direction until explicitly stopped. Use this when the user says 'walk until I stop you', 'keep going', 'walk forward', etc.",
            "parameters": {
                "type": "object",
                "properties": {
                    "direction": {
                        "type": "string",
                        "enum": ["forward", "backward", "left", "right", "turn_left", "turn_right"],
                        "description": "Direction of movement"
                    },
                    "speed": {
                        "type": "number",
                        "description": "Speed multiplier (0.5 to 2.0), default 1.0"
                    }
                },
                "required": ["direction"]
            },
        }
    },
    {
        "type": "function",
        "function": {
            "name": "move",
            "description": "Move the robot in a specific direction for a set duration, then stop automatically. Use this when the user specifies a time or distance.",
            "parameters": {
                "type": "object",
                "properties": {
                    "direction": {
                        "type": "string",
                        "enum": ["forward", "backward", "left", "right", "turn_left", "turn_right"],
                        "description": "Direction of movement"
                    },
                    "duration": {
                        "type": "number",
                        "description": "Duration in seconds to move before stopping"
                    },
                    "speed": {
                        "type": "number",
                        "description": "Speed multiplier (0.5 to 2.0), default 1.0"
                    }
                },
                "required": ["direction", "duration"]
            },
        }
    },
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
                "properties": {
                    "bias_points": {
                        "type": "array",
                        "description": "Optional list of world coordinates to bias exit search toward.",
                        "items": {
                            "type": "array",
                            "items": [{"type": "number"}, {"type": "number"}],
                            "minItems": 2,
                            "maxItems": 2
                        }
                    }
                },
            },
        }
    },
    {
        "type": "function",
        "function": {
            "name": "navigate_waypoints",
            "description": "Navigate through a list of waypoints in order.",
            "parameters": {
                "type": "object",
                "properties": {
                    "waypoints": {
                        "type": "array",
                        "items": {
                            "type": "array",
                            "items": [{"type": "number"}, {"type": "number"}],
                            "minItems": 2,
                            "maxItems": 2
                        }
                    }
                },
                "required": ["waypoints"]
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
    },
    {
        "type": "function",
        "function": {
            "name": "look",
            "description": "Capture a camera frame and describe what it sees.",
            "parameters": {
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Optional prompt for what to look for"}
                },
            },
        }
    }
]
