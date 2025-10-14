# Audio Crate Examples

## test_tts.rs

Tests the Text-to-Speech (TTS) functionality with caching.

### Prerequisites

1. Make sure you have a TTS server running (e.g., Piper TTS)
2. Update the `tts_url` in the example if your server is not at `http://localhost:5002/api/tts`

### Running the Example

```bash
# With real TTS (requires TTS server)
cargo run --example test_tts --features real

# With dummy TTS (for testing without server)
cargo run --example test_tts --features dummy
```

### What it does

1. Initializes the TTS system
2. Says "Hello World" in English (first time - generates and caches)
3. Says "Hello World" again (second time - uses cache on subsequent runs)
4. Says "Ahoj svet" in Slovak

### Cache Location

- Cache metadata: `/tmp/hexapod/tts/cache.toml`
- Cache files: `/tmp/hexapod/tts/cache/*.wav`

The cache persists between runs, so:
- First run: Generates TTS twice for each phrase
- Second run: Uses cached files directly (no TTS generation needed)
