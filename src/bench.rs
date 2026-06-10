use std::time::Duration;

use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::prelude::*;
use clap::Parser;

use crate::player::{FlyActivation, Player, PlayerController, PlayerSettings};
use crate::AppState;

/// Command-line options for profiling and automated benchmarks.
#[derive(Parser, Debug, Resource, Clone)]
#[command(name = "bridgetcraft", about = "BridgetCraft voxel creative game")]
pub struct CliArgs {
    /// Log frame-time diagnostics to the terminal every second.
    #[arg(long)]
    pub diag_log: bool,

    /// Run an automated benchmark: skip the menu, render distance 6, scripted fly path.
    #[arg(long)]
    pub bench: bool,

    /// Seconds to run the benchmark before printing a summary and exiting (0 = run until quit).
    #[arg(long, default_value_t = 0.0)]
    pub bench_duration: f32,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            diag_log: false,
            bench: false,
            bench_duration: 0.0,
        }
    }
}

pub struct BenchPlugin;

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        let args = CliArgs::parse();
        let bench = args.bench;
        let bench_duration = args.bench_duration;

        if args.diag_log || bench {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }

        if args.diag_log {
            app.add_plugins(LogDiagnosticsPlugin {
                wait_duration: Duration::from_secs(1),
                ..default()
            });
        }

        app.insert_resource(args);

        if bench {
            app.insert_resource(BenchState::new(bench_duration));
            app.add_systems(Startup, configure_bench_settings);
            app.add_systems(Update, (start_bench_game, bench_fly_path, bench_report).chain());
        }
    }
}

#[derive(Resource)]
struct BenchState {
    started: bool,
    duration: f32,
    elapsed: f32,
    reported: bool,
}

impl BenchState {
    fn new(duration: f32) -> Self {
        Self {
            started: false,
            duration,
            elapsed: 0.0,
            reported: false,
        }
    }
}

fn configure_bench_settings(mut settings: ResMut<PlayerSettings>) {
    settings.render_distance = 6;
    settings.fly_activation = FlyActivation::Always;
    settings.show_diagnostics = true;
}

fn start_bench_game(
    mut bench: ResMut<BenchState>,
    mut next_state: ResMut<NextState<AppState>>,
    state: Res<State<AppState>>,
    mut overlay: ResMut<bevy::dev_tools::fps_overlay::FpsOverlayConfig>,
) {
    if bench.started || *state.get() == AppState::InGame {
        return;
    }

    bench.started = true;
    overlay.enabled = true;
    overlay.frame_time_graph_config.enabled = true;
    next_state.set(AppState::InGame);
    info!(
        "benchmark mode: render distance 6, scripted fly path{}",
        if bench.duration > 0.0 {
            format!(", duration {:.1}s", bench.duration)
        } else {
            String::new()
        }
    );
}

fn bench_fly_path(
    time: Res<Time>,
    bench: Res<BenchState>,
    mut players: Query<(&mut Transform, &mut PlayerController), With<Player>>,
) {
    if !bench.started {
        return;
    }

    let Ok((mut transform, mut controller)) = players.single_mut() else {
        return;
    };

    controller.flying = true;
    controller.grounded = false;
    controller.velocity = Vec3::ZERO;

    let t = time.elapsed_secs();
    let radius = 48.0;
    let height = 18.0;
    let speed = 0.35;
    transform.translation = Vec3::new(
        radius * (t * speed).cos(),
        height + (t * 0.7).sin() * 4.0,
        radius * (t * speed).sin(),
    );
    controller.yaw = t * speed + std::f32::consts::FRAC_PI_2;
}

fn bench_report(
    time: Res<Time>,
    mut bench: ResMut<BenchState>,
    mut exit: MessageWriter<AppExit>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) {
    if bench.duration <= 0.0 || bench.reported {
        return;
    }

    bench.elapsed += time.delta_secs();
    if bench.elapsed < bench.duration {
        return;
    }

    bench.reported = true;
    if let Some(fps) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
            info!("benchmark complete: avg FPS {:.1}", value);
        } else if let Some(value) = fps.value() {
            info!("benchmark complete: FPS {:.1}", value);
        }
    }
    if let Some(frame_time) =
        diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME)
    {
        if let Some(ms) = frame_time.smoothed() {
            info!("benchmark complete: frame time {:.2} ms", ms);
        }
    }
    exit.write(AppExit::Success);
}
