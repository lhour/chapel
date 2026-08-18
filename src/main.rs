//! A small half-ruined chapel scene — a self-contained Bevy 0.19 demonstration that
//! reuses the warbell (tileworld) visual recipe: primitives → vertex-colour → merge →
//! flat shade, all sharing a single white standard material so the whole scene batches.

mod chapel;
mod env;
mod geoms;

use bevy::prelude::*;
use bevy::pbr::DistanceFog;
use bevy::render::view::Hdr;
use bevy::light::{DirectionalLight, NotShadowCaster};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Half-Ruined Chapel".into(),
                ..default()
            }),
            ..default()
        }))
        // Pale daytime sky + warm distance haze (warbell-style finish) so the scene is not a
        // black void and the ruins pick up atmosphere.
        .insert_resource(ClearColor(Color::srgb(0.68, 0.80, 0.92)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.62, 0.68, 0.8),
            brightness: 900.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, orbit_camera)
        .run();
}

/// Build the shared white vertex-colour material that every mesh uses (batches everything).
fn white_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    })
}

/// Spawn each prebuilt mesh as its own entity sharing the one white material.
fn spawn_all(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mat: &Handle<StandardMaterial>,
    built: Vec<Mesh>,
) {
    for m in built {
        let h = meshes.add(m);
        commands.spawn((
            Mesh3d(h),
            MeshMaterial3d(mat.clone()),
            Transform::default(),
            NotShadowCaster,
        ));
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mat = white_material(&mut *materials);

    // ── Lighting (warbell-style warm daylight + generous ambient for the ruined look) ──
    commands.spawn((
        DirectionalLight {
            illuminance: 40_000.0,
            color: Color::srgb(1.0, 0.96, 0.88),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.9, -0.3)),
    ));

    // ── Ground + environment ───────────────────────────────────────────────
    let ground = meshes.add(env::build_ground());
    commands.spawn((Mesh3d(ground), MeshMaterial3d(mat.clone()), Transform::default()));
    spawn_all(&mut commands, &mut *meshes, &mat, env::build_grass());
    spawn_all(&mut commands, &mut *meshes, &mat, env::build_rubble_flat());

    // trees (each is its own mesh, shared handle)
    let tree = meshes.add(env::build_tree());
    for (x, z, s) in env::tree_placements() {
        commands.spawn((
            Mesh3d(tree.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(s)),
        ));
    }

    // ── Chapel (casts shadows) ────────────────────────────────────────────
    for m in chapel::build_chapel() {
        let h = meshes.add(m);
        commands.spawn((
            Mesh3d(h),
            MeshMaterial3d(mat.clone()),
            Transform::default(),
        ));
    }

    // ── Camera ────────────────────────────────────────────────────────────
    commands.spawn((
        Orbit { radius: 20.0, yaw: 0.0, pitch: 0.45 },
        Camera3d::default(),
        Transform::from_xyz(14.0, 9.0, 18.0).looking_at(Vec3::new(0.0, 2.5, 0.0), Vec3::Y),
        Hdr,
        // soft warm haze at the horizon so the cleared sky & ground blend into a sunlit day.
        DistanceFog {
            color: Color::srgb(0.72, 0.75, 0.70),
            directional_light_color: Color::srgb(0.95, 0.92, 0.84),
            directional_light_exponent: 6.0,
            falloff: bevy::pbr::FogFalloff::Linear {
                start: 46.0,
                end: 90.0,
            },
            ..default()
        },
    ));
}

/// A component marking the single orbit camera.
#[derive(Component)]
struct Orbit {
    radius: f32,
    yaw: f32,
    pitch: f32,
}

/// Slow orbit around the chapel so every side (including the ruined apse & tower) is visible.
fn orbit_camera(time: Res<Time>, mut q: Query<(&mut Transform, &mut Orbit), With<Camera3d>>) {
    for (mut tf, mut orbit) in &mut q {
        orbit.yaw += time.delta_secs() * 0.15;
        let t = orbit;
        let dir = Vec3::new(
            t.pitch.cos() * t.yaw.sin(),
            t.pitch.sin(),
            t.pitch.cos() * t.yaw.cos(),
        );
        let pos = Vec3::new(0.0, 2.5, -2.0) + dir * t.radius;
        tf.translation = pos;
        tf.look_at(Vec3::new(0.0, 2.5, -2.0), Vec3::Y);
    }
}