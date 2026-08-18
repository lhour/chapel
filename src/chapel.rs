//! Chapel model — a half-ruined stone chapel: nave + east apse + west tower.
//! Walls are built as coursed stone blocks (each block a tinted cuboid, merged into a
//! handful of meshes so everything batches). Ruin is procedural but deterministic: a simple
//! hash decides which top-course blocks are missing and how ragged each wall's top is.

use bevy::prelude::*;
use bevy::mesh::Mesh;

use crate::geoms::*;

// ── Layout (world units; floor plane at y = 0) ──────────────────────────────
pub const NAVE_W: f32 = 10.0; // nave width (X)
pub const NAVE_L: f32 = 16.0; // nave length (Z)
pub const WALL_H: f32 = 6.0; // full wall height
pub const WALL_T: f32 = 0.8; // wall thickness
pub const BLOCK_W: f32 = 1.0; // stone block footprint
pub const BLOCK_H: f32 = 0.5; // stone course height
pub const APSE_R: f32 = 4.0; // apse radius
pub const TOWER_W: f32 = 5.0;
pub const TOWER_H: f32 = 10.0;

/// Small deterministic pseudo-random in [0,1) from an integer key (so ruins are reproducible).
fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x1f1f1f1f) ^ ((y as u32) << 7);
    h = h.wrapping_mul(0x1f1f1f1f);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

struct StoneCols {
    light: [f32; 4],
    mid: [f32; 4],
    dark: [f32; 4],
}

fn stone_cols() -> StoneCols {
    StoneCols { light: lin(STONE_LIGHT), mid: lin(STONE), dark: lin(STONE_DARK) }
}

/// Pick a stone colour with a little per-block variation.
fn pick(c: &StoneCols, x: i32, y: i32) -> [f32; 4] {
    let r = hash2(x, y * 31 + 7);
    if r < 0.18 { c.dark } else if r < 0.55 { c.mid } else { c.light }
}

/// Build a rectangular wall made of coursed blocks, standing at a given base line.
/// `along` (X or Z) is the axis the wall runs along; `len` its span; `half` the transverse
/// offset from the origin; `drop_frac` controls how much of the upper courses is missing.
/// Returns the mesh and a count of placed blocks (for rubble estimation).
fn coursed_wall(
    along: char,
    base_x: f32,
    base_z: f32,
    len: f32,
    wall_h: f32,
    drop: f32,
    cols: &StoneCols,
    parts: &mut Vec<(Mesh, [f32; 4])>,
) {
    let courses = (wall_h / BLOCK_H).ceil() as i32;
    let nblocks = (len / BLOCK_W).ceil() as i32;
    for cy in 0..courses {
        let y = cy as f32 * BLOCK_H + (BLOCK_H * 0.5);
        for bx in 0..nblocks {
            // running-bond: alternate course starts offset by half a block.
            let off = if cy % 2 == 0 { 0.0 } else { BLOCK_W * 0.5 };
            let along_offs = bx as f32 * BLOCK_W + off;
            if along_offs > len { continue; }
            // ruin profile: missing blocks concentrate toward the top, ragged edge.
            let top_frac = cy as f32 / courses.max(1) as f32;
            let r = hash2(bx, cy);
            let miss = if r < 0.5 && top_frac > 1.0 - drop { true } else { r < 0.05 };
            if miss { continue; }
            let (px, pz) = match along {
                'x' => (base_x, base_z + along_offs),
                _ => (base_x + along_offs, base_z),
            };
            let mut m = match along {
                'x' => cuboid(WALL_T, BLOCK_H * 0.9, BLOCK_W * 0.92),
                _ => cuboid(BLOCK_W * 0.92, BLOCK_H * 0.9, WALL_T),
            };
            m = m.translated_by(Vec3::new(px, y, pz));
            parts.push((m, pick(cols, bx, cy)));
        }
    }
}

/// Build the full chapel model and return the finished meshes (already merged+flat+coloured).
pub fn build_chapel() -> Vec<Mesh> {
    let cols = stone_cols();

    // ── Nave walls ──────────────────────────────────────────────────────────
    let mut nave: Vec<(Mesh, [f32; 4])> = Vec::new();
    // north & south long walls run along Z at x = ±W/2.
    coursed_wall('z', -NAVE_W / 2.0, -NAVE_L / 2.0, NAVE_L, WALL_H, 0.35, &cols, &mut nave);
    coursed_wall('z', NAVE_W / 2.0, -NAVE_L / 2.0, NAVE_L, WALL_H, 0.20, &cols, &mut nave);
    // west wall (front, with an arched door opening left roughly in the middle) and east wall.
    coursed_wall('x', -NAVE_L / 2.0, -NAVE_W / 2.0, NAVE_W, WALL_H, 0.45, &cols, &mut nave);
    coursed_wall('x', NAVE_L / 2.0, -NAVE_W / 2.0, NAVE_W, WALL_H, 0.25, &cols, &mut nave);

    // ── Apse (east end): a short polygonal half-round drum of coursed blocks ─
    let apse_seg = 8usize;
    for seg in 0..apse_seg {
        let th0 = (seg as f32 / apse_seg as f32) * std::f32::consts::PI;
        if th0 > std::f32::consts::PI * 0.5 { continue; }
        let th1 = ((seg + 1) as f32 / apse_seg as f32) * std::f32::consts::PI;
        let thc = (th0 + th1) * 0.5;
        // drum sits at the east end, bulging +Z from the nave.
        let cx = thc.sin() * APSE_R + NAVE_L / 2.0 - 0.4;
        let cz = thc.cos() * -APSE_R;
        let cw = ((th1 - th0) * APSE_R).abs().max(0.6);
        for cy in 0..((WALL_H / BLOCK_H).ceil() as i32) {
            let y = cy as f32 * BLOCK_H + BLOCK_H * 0.5;
            let top_frac = cy as f32 / (WALL_H / BLOCK_H).floor().max(1.0);
            if hash2(seg as i32, cy) < 0.35 && top_frac > 0.7 { continue; }
            let mut m = cuboid(cw, BLOCK_H * 0.9, WALL_T * 0.9);
            m = m.translated_by(Vec3::new(-cz, y, cx)); // note: cz is negative for the +Z bulge
            m = m.rotated_by(Quat::from_rotation_y(thc));
            nave.push((m, pick(&cols, seg as i32, cy)));
        }
    }

    // ── West tower ──────────────────────────────────────────────────────────
    // A squat square tower rising above a corner of the west front. Its top course is
    // crenellated and partly collapsed.
    let tw = TOWER_W;
    let tx = -NAVE_L / 2.0 - 1.0; // tower base at west, offset a little south of the centerline
    let tz = -NAVE_W / 2.0 + 1.0;
    let courses = (TOWER_H / BLOCK_H).ceil() as i32;
    for which in ['x', 'z'] {
        let along_len = if which == 'x' { tw } else { tw };
        let base = match which { 'x' => tz, _ => tx };
        let fix = match which { 'x' => tx, _ => tz };
        let mut tv: Vec<(Mesh, [f32; 4])> = Vec::new();
        for k in 0..courses {
            let y = k as f32 * BLOCK_H + BLOCK_H * 0.5;
            let top_frac = k as f32 / courses.max(1) as f32;
            // ragged crown: collapse starts above ~80%
            if top_frac > 0.8 {
                // thin a ragged ring
                if hash2(k, which as u8 as i32 * 3) < 0.6 { continue; }
            }
            for b in 0..((along_len / BLOCK_W).ceil() as i32) {
                let off = if k % 2 == 0 { 0.0 } else { BLOCK_W * 0.5 };
                let p = b as f32 * BLOCK_W + off;
                if p > along_len { continue; }
                let (px, pz) = if which == 'x' {
                    (base + p, fix)
                } else {
                    (fix, base + p)
                };
                let mut m = if which == 'x' {
                    cuboid(BLOCK_W * 0.92, BLOCK_H * 0.9, WALL_T)
                } else {
                    cuboid(WALL_T, BLOCK_H * 0.9, BLOCK_W * 0.92)
                };
                m = m.translated_by(Vec3::new(px, y, pz));
                tv.push((m, pick(&cols, b, k * 5 + which as u8 as i32)));
            }
        }
        // tower corners use darker stone.
        for k in 0..courses {
            let y = k as f32 * BLOCK_H + BLOCK_H * 0.5;
            for (cx0, cz0) in [(tx, tz), (tx, tz + tw), (tx + tw, tz), (tx + tw, tz + tw)] {
                let mut m = cuboid(BLOCK_W * 0.9, BLOCK_H * 0.9, BLOCK_W * 0.9);
                m = m.translated_by(Vec3::new(cx0 - 0.1, y, cz0 - 0.1));
                tv.push((m, cols.dark));
            }
        }
        nave.extend(tv);
    }

    // ── Broken roof (a partial west-inclined shed roof over the nave) ───────
    // A broad low roof sheet leaning from the north wall toward the south wall, with a
    // giant bite missing toward the east + a couple of holes.
    let roof_w = NAVE_W * 0.55;
    let roof_l = NAVE_L * 0.9;
    let roof_start_z = -NAVE_L / 2.0 + NAVE_L * 0.1;
    // roof as a tilted slab
    let mut roof = cuboid(roof_w, 0.15, roof_l);
    roof = roof.translated_by(Vec3::new(-NAVE_W * 0.2, WALL_H + 1.2, roof_start_z));
    roof = roof.rotated_by(Quat::from_rotation_z(0.28));
    nave.push((roof, lin(ROOF)));

    // a few roof supports / split rafters poking up
    for i in 0..3 {
        let z = -NAVE_L / 2.0 + 2.0 + i as f32 * 4.0;
        let mut beam = cuboid(0.14, 0.5, 2.6);
        beam = beam.translated_by(Vec3::new(-NAVE_W * 0.15, WALL_H - 0.1, z));
        beam = beam.rotated_by(Quat::from_rotation_x(0.6));
        nave.push((beam, lin(WOOD)));
    }

    // ── West door (dark opening in the front wall) ──────────────────────────
    {
        let mut door = cuboid(1.6, 3.0, 0.2);
        door = door.translated_by(Vec3::new(-NAVE_L / 2.0 + 0.1, 1.5, NAVE_W / 2.0 - 1.0));
        door = door.rotated_by(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        nave.push((door, lin(DOOR)));
    }

    //── scattered rubble at the base of the broken walls ─────────────────────
    for i in 0..26 {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let x = side * (NAVE_W / 2.0 - 0.6) + (hash2(i, 3) - 0.5) * 2.5;
        let z = -NAVE_L / 2.0 * 0.5 + (hash2(i, 9) - 0.5) * NAVE_L;
        let s = 0.15 + hash2(i, 11) * 0.25;
        let mut m = cube(s);
        m = m.translated_by(Vec3::new(x, s * 0.4, z));
        m = m.rotated_by(Quat::from_euler(EulerRot::XYZ, hash2(i, 1) * 3.0, 0.0, hash2(i, 2) * 3.0));
        let c = if hash2(i, 5) < 0.4 { BRICK } else { STONE_DARK };
        nave.push((m, lin(c)));
    }

    // Merge the nave assembly into a small number of draw calls: group into ~4 meshes.
    let mut out = Vec::new();
    // chunks of ~150 parts to keep the merged meshes reasonable in size.
    let mut cur: Vec<(Mesh, [f32; 4])> = Vec::new();
    for p in nave {
        cur.push(p);
        if cur.len() >= 150 {
            out.push(mesh(std::mem::take(&mut cur)));
        }
    }
    if !cur.is_empty() { out.push(mesh(cur)); }
    out
}