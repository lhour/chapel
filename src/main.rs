//! Romanesque Basilica 场景——方案 B：
//! 唯一真源 = assets/basilica.json（参数化蓝图）。
//! Dispatcher（src/blueprint.rs）按蓝图调用砖砌原语库（src/basilica.rs）。
//! 所有砖是逐块 Cuboid / 楔形砖，合并后共享单一白色材质 → 自动合批。

mod basilica;
mod blueprint;
mod chapel;
mod env;
mod geoms;

use bevy::camera::Hdr;
use bevy::light::{DirectionalLight, NotShadowCaster};
use bevy::pbr::DistanceFog;
use bevy::prelude::*;

/// 编译期嵌入蓝图 JSON（避免运行时 cwd/路径依赖）。
const BASILICA_JSON: &str = include_str!("../assets/basilica.json");

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Romanesque Basilica · Plan B".into(),
                ..default()
            }),
            ..default()
        }))
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

fn white_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    })
}

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

    // ── 光照：暖日光 + 柔和环境光 ────────────────────────────────────────
    commands.spawn((
        DirectionalLight {
            illuminance: 40_000.0,
            color: Color::srgb(1.0, 0.96, 0.88),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.9, -0.3)),
    ));

    // ── 地面 + 环境（草地/碎石/树）────────────────────────────────────────
    let ground = meshes.add(env::build_ground());
    commands.spawn((Mesh3d(ground), MeshMaterial3d(mat.clone()), Transform::default()));
    spawn_all(&mut commands, &mut *meshes, &mat, env::build_grass());
    spawn_all(&mut commands, &mut *meshes, &mat, env::build_rubble_flat());

    let tree = meshes.add(env::build_tree());
    for (x, z, s) in env::tree_placements() {
        commands.spawn((
            Mesh3d(tree.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(s)),
        ));
    }

    // ── 方案 B：加载蓝图 → 解析 → Dispatcher → 原语砌砖 → 合批 spawn ──────
    {
        let bp = blueprint::parse(BASILICA_JSON);
        let cols = geoms::stone_cols();
        let mut parts: basilica::Parts = Vec::new();
        blueprint::dispatch_all(&bp, &cols, &mut parts);

        // 教堂主体投射阴影（不添加 NotShadowCaster）
        let meshes_merged = blueprint::merge_parts(parts);
        for m in meshes_merged {
            let h = meshes.add(m);
            commands.spawn((
                Mesh3d(h),
                MeshMaterial3d(mat.clone()),
                Transform::default(),
            ));
        }
    }

    // ── 相机 + 雾（距离雾霾，暖色调与 project_memory 约定对齐）──────────
    commands.spawn((
        Orbit { radius: 46.0, yaw: 0.2, pitch: 0.36 },
        Camera3d::default(),
        Transform::from_xyz(30.0, 15.0, 34.0).looking_at(Vec3::new(0.0, 6.0, 0.0), Vec3::Y),
        Hdr,
        DistanceFog {
            color: Color::srgb(0.72, 0.75, 0.70),
            directional_light_color: Color::srgb(1.0, 0.95, 0.86),
            directional_light_exponent: 6.0,
            falloff: bevy::pbr::FogFalloff::Linear {
                start: 26.0,   // 约定: Linear start=26, end=86
                end:   86.0,
            },
            ..default()
        },
    ));
}

#[derive(Component)]
struct Orbit {
    radius: f32,
    yaw: f32,
    pitch: f32,
}

/// 镜头缓慢环绕教堂一周，让各立面、后殿、双塔都被看到。
fn orbit_camera(time: Res<Time>, mut q: Query<(&mut Transform, &mut Orbit), With<Camera3d>>) {
    for (mut tf, mut orbit) in &mut q {
        orbit.yaw += time.delta_secs() * 0.08;
        let t = &*orbit;
        let dir = Vec3::new(
            t.pitch.cos() * t.yaw.sin(),
            t.pitch.sin(),
            t.pitch.cos() * t.yaw.cos(),
        );
        let pos = Vec3::new(0.0, 6.0, 0.0) + dir * t.radius;
        tf.translation = pos;
        tf.look_at(Vec3::new(0.0, 6.0, 0.0), Vec3::Y);
    }
}
