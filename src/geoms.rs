//! Palette + low-poly mesh helpers, modeled on the warbell (tileworld) recipe:
//! primitive → tint(colour) → merge → flat_shade, all against a single white
//! StandardMaterial so thousands of parts auto-batch.

use bevy::math::primitives::*;
use bevy::mesh::Mesh;
use bevy::prelude::*;

/// sRGB Color from a 0xRRGGBB literal (material base colours).
pub fn srgb(hex: u32) -> Color {
    Color::srgb_u8(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}

/// Linear [r,g,b,1] for mesh ATTRIBUTE_COLOR.
pub fn lin(hex: u32) -> [f32; 4] {
    let l = srgb(hex).to_linear();
    [l.red, l.green, l.blue, 1.0]
}

/// Linear colour scaled by `v` (per-part brightness tint).
pub fn lin_scaled(hex: u32, v: f32) -> [f32; 4] {
    let l = srgb(hex).to_linear();
    [l.red * v, l.green * v, l.blue * v, 1.0]
}

// ── Chapel + ground palette ────────────────────────────────────────────────
pub const STONE_LIGHT: u32 = 0xb9b5a8; // weathered light stone
pub const STONE: u32 = 0x9a968a; // mid stone
pub const STONE_DARK: u32 = 0x6f6b62; // shadowed stone / mortar
pub const BRICK: u32 = 0x8f5a45; // scattered brick rubble
pub const BRICK_LIGHT: u32 = 0xa9725c;
pub const WOOD: u32 = 0x5a3a22; // rotting timber
pub const WOOD_DARK: u32 = 0x3f2a18;
pub const ROOF: u32 = 0x5a4a3a; // dark roof slate
pub const ROOF_LIGHT: u32 = 0x6d5a44;
pub const GROUND: u32 = 0x6b6b5e; // packed earth
pub const GRASS: u32 = 0x5c7a4a; // a little vegetation
pub const DOOR: u32 = 0x4a3320; // heavy door boards

/// Helper build contract — see warbell's meshkit.rs. A primitive → vertex colour → merge →
/// flat shade. All parts share one white material (batched).
/// Merges `parts` (already tinted) then flat-shades (duplicate first!).
pub fn merged_flat(parts: Vec<Mesh>) -> Mesh {
    let mut it = parts.into_iter();
    let mut base = it.next().expect("at least one part");
    for p in it {
        base.merge(&p).expect("parts share attributes");
    }
    base.duplicate_vertices();
    base.compute_flat_normals();
    base
}

/// Tag every vertex of `m` with one flat linear colour. REQUIRED before a merge.
pub fn tinted(mut m: Mesh, c: [f32; 4]) -> Mesh {
    let n = m.count_vertices();
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![c; n]);
    m
}

/// tinted from a packed `0xRRGGBB`.
pub fn t(m: Mesh, hex: u32) -> Mesh {
    tinted(m, lin(hex))
}

/// One merged+flat+coloured mesh from the given (mesh,colour) parts, sharing one batch.
pub fn mesh(parts: Vec<(Mesh, [f32; 4])>) -> Mesh {
    merged_flat(parts.into_iter().map(|(m, c)| tinted(m, c)).collect())
}

// ── Primitives (Bevy 0.19 mesh builders) ───────────────────────────────────
pub fn cuboid(sx: f32, sy: f32, sz: f32) -> Mesh {
    Cuboid::new(sx, sy, sz).mesh().build()
}
pub fn cube(s: f32) -> Mesh {
    cuboid(s, s, s)
}
/// Tapered cylinder; rt==rb -> plain cylinder, rt==0 -> cone.
pub fn frustum(rt: f32, rb: f32, h: f32, res: u32) -> Mesh {
    ConicalFrustum { radius_top: rt, radius_bottom: rb, height: h }.mesh().resolution(res).build()
}
pub fn cone(r: f32, h: f32, res: u32) -> Mesh {
    Cone { radius: r, height: h }.mesh().resolution(res).build()
}
pub fn ball(r: f32) -> Mesh {
    Sphere::new(r).mesh().ico(2).unwrap()
}
pub fn slab(w: f32, h: f32) -> Mesh {
    Cuboid::new(w, h, 0.02).mesh().build()
}