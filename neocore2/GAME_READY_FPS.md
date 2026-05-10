# Game Ready FPS vertical slice

This patch adds a deliberately hardcoded but playable first-person slice:

- separate launcher: `apps/game-ready-fps`
- env switch: `NEWENGINE_GAME_READY_DEMO=1`
- immediate Play mode instead of editor-first startup
- hardcoded arena scene with floor, walls, crates, pickups, hazards and extraction beacon
- player spawn with first-person camera possession
- HUD with objective/progress/status
- gameplay loop: collect 3 cores -> reach beacon -> win; touch hazard -> fail

## Run

```bash
cargo run -p game-ready-fps
```

## Controls

- `WASD` — move
- Mouse — look
- `Shift` — sprint
- `ESC` — exit Play back to editor

## Design intent

This is intentionally not a clean content pipeline yet. The goal is to force the engine to serve gameplay immediately:

1. boot directly into something playable;
2. keep all game-demo logic isolated from the normal editor flow;
3. reuse existing NewEngine runtime pieces instead of inventing another mini-engine;
4. leave the hardcoded layer as a replaceable prototype seam.
