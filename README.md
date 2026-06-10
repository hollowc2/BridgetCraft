# BridgetCraft

A Rust voxel creative game for parent-and-child LAN play, inspired by Minecraft. Built with [Bevy](https://bevyengine.org/) and [bevy_voxel_world](https://github.com/splashdust/bevy_voxel_world).

## Features

- 3D first-person creative mode — walk, fly, place and break blocks
- Procedural hills and trees
- ~20 block types from the [Kenney Voxel Pack](https://kenney.nl/assets/voxel-pack)
- LAN multiplayer — host on one machine, join from another
- Auto-save world edits to `~/.local/share/BridgetCraft/worlds/`

## Requirements

- Rust 1.85+ (edition 2021)
- Linux with a GPU supporting Vulkan or OpenGL
- For LAN play: both machines on the same local network

## Build & Run

```bash
cargo run
```

Release build (faster):

```bash
cargo run --release
```

Fast iteration (dynamic Bevy linking):

```bash
cargo run --features dynamic_linking
```

### Performance profiling

```bash
# Log frame-time stats to the terminal every second
cargo run --release -- --diag-log

# Automated benchmark: skip menu, render distance 6, scripted fly path
cargo run --release -- --bench --bench-duration 30

# Tracy flamegraphs (requires tracy profiler)
cargo run --release --features trace_tracy
```

The build script generates `assets/textures/voxel_atlas.png` from Kenney tile PNGs automatically.

## Controls

| Input | Action |
|-------|--------|
| WASD | Move |
| Mouse | Look around |
| Space | Jump / fly up |
| Shift | Fly down (while flying) |
| Double-tap Space | Toggle creative flight |
| Left click | Break block |
| Right click | Place block |
| 1–9 | Select hotbar slot |
| Scroll wheel | Cycle hotbar |
| Escape | Return to main menu |

## Multiplayer (LAN)

1. On the host machine, click **Host Game**. Note the IP and port shown (default `7777`).
2. On the other machine, click **Join Game** and enter `HOST_IP:7777`.
3. If the joiner cannot connect, allow UDP port 7777 through the host firewall:

```bash
sudo ufw allow 7777/udp
```

## Project Structure

```
src/
  main.rs           App entry, game states
  block.rs          Block types and texture atlas indices
  voxel_config.rs   bevy_voxel_world configuration
  world_gen.rs      Procedural terrain and trees
  player.rs         Movement, flight, collision
  interaction.rs    Block place/break and raycasting
  save.rs           Auto-save and world loading
  ui/               Main menu and HUD
  net/              LAN host/join and replication
assets/
  kenney_voxel-pack/  CC0 voxel art (Kenney.nl)
```

## License

Game code: MIT OR Apache-2.0 (your choice).

Assets: [Kenney Voxel Pack](assets/kenney_voxel-pack/License.txt) — CC0 1.0 Universal.
