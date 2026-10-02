# Neon Breakout

A tiny breakout game written in Rust with [macroquad](https://macroquad.rs). Neon shapes, glowing bricks, particle bursts, and every brick plays a note from a pentatonic scale. All sounds are generated in code, so there are no asset files.

## Controls

| Action | Keys |
| --- | --- |
| Move paddle | Mouse, or Left/Right, or A/D |
| Launch ball | Space or click |
| Restart | R (or Space on the end screen) |

## Run it

You need [Rust](https://rustup.rs). On Ubuntu/Debian, install the system libs first:

```bash
sudo apt install pkg-config libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev
```

Then:

```bash
cargo run --release
```

## Ideas to extend it

- Power-ups (wider paddle, multi-ball)
- Levels with different brick layouts
- High score saved to a file
- Web build with WebAssembly
