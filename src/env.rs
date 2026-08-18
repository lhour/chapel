//! Environment around the chapel: a packed-earth forecourt, patches of grass, some
//! broken flagstones, and a few low-poly trees (recreating warbell's tree look).

use bevy::prelude::*;
use bevy::mesh::Mesh;

use crate::geoms::*;

/// Build the ground plane (the ground colour lives in vertex colours so it batches with the
/// chapel if we kept it simple — but we tint it via a plain mesh for clarity).
pub fn build_ground() -> Mesh {
    let mut g = cuboid(60.0, 0.4, 60.0);
    g = g.translated_by(Vec3::new(0.0, -0.2, 0.0));
    t(g, GROUND)
}

/// A cluster of green tufts / grass patches scatter.
pub fn build_grass() -> Vec<Mesh> {
    // Represented as low flattened stone/grass discs so they read from a distance.
    let mut out = Vec::new();
    for i in 0..30 {
        let x = (hash2(i, 21) - 0.5) * 34.0;
        let z = (hash2(i, 22) - 0.5) * 34.0;
        // keep the forecourt clear near chapel centre
        let dist = Vec2::new(x, z).length();
        if dist < 10.0 { continue; }
        let r = 0.5 + hash2(i, 23) * 0.9;
        let mut m = cuboid(r, 0.08, r * 0.7);
        m = m.translated_by(Vec3::new(x, 0.02, z));
        m = m.rotated_by(Quat::from_rotation_y(hash2(i, 24) * 3.0));
        out.push(t(m, GRASS));
    }
    out
}

/// Scattered broken flagstones / chipped paving slabs near the chapel.
pub fn build_rubble_flat() -> Vec<Mesh> {
    let mut out = Vec::new();
    for i in 0..14 {
        let ang = hash2(i, 31) * 6.283;
        let rad = 4.0 + hash2(i, 32) * 3.0;
        let x = ang.cos() * rad;
        let z = ang.sin() * rad;
        let w = 0.4 + hash2(i, 33) * 0.4;
        let mut m = slab(w, w * (0.5 + hash2(i, 34)));
        m = m.translated_by(Vec3::new(x, 0.03, z));
        m = m.rotated_by(Quat::from_rotation_y(hash2(i, 35) * 6.283));
        out.push(t(m, STONE_DARK));
    }
    out
}

/// Build one low-poly tree (trunk + two foliage layers) in warbell style.
pub fn build_tree() -> Mesh {
    let trunk = frustum(0.18, 0.28, 2.2, 6);
    let trunk = trunk.translated_by(Vec3::new(0.0, 1.1, 0.0));
    let crown = cone(1.3, 2.6, 7).translated_by(Vec3::new(0.0, 3.2, 0.0));
    let crown2 = cone(0.95, 2.0, 7).translated_by(Vec3::new(0.15, 4.0, 0.0));
    mesh(vec![
        (trunk, lin(WOOD)),
        (crown, lin(0x3c7a3e)),
        (crown2, lin(0x4f9a4a)),
    ])
}

/// Return (x, z) placements + scale for a ring of trees.
pub fn tree_placements() -> Vec<(f32, f32, f32)> {
    let mut v = Vec::new();
    for i in 0..9 {
        let ang = i as f32 * 2.399 + hash2(i, 41) * 0.4;
        let rad = 14.0 + hash2(i, 42) * 6.0;
        let sx = 0.8 + hash2(i, 43) * 0.6;
        v.push((ang.cos() * rad, ang.sin() * rad, sx));
    }
    v
}

fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x1f1f1f1f) ^ ((y as u32) << 7);
    h = h.wrapping_mul(0x1f1f1f1f);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}