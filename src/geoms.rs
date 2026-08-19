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

/// 半圆锥（半圆后殿锥顶专用）：底面为 XZ 平面上 θ∈[-π/2,+π/2] 的半圆
/// （平边直径沿 Z 轴，弧朝 +X），顶点在 (0,h,0)。
/// 平背面（直径三角面）法线朝 -X（贴向建筑主体的横厅东墙）。
pub fn half_cone(r: f32, h: f32, res: u32) -> Mesh {
    let n = res as usize;
    let mut verts: Vec<[f32; 3]> = Vec::with_capacity(n + 2);
    let mut indices: Vec<u32> = Vec::with_capacity(n * 3 + 3 * (n - 1));
    verts.push([0.0, h, 0.0]); // apex = index 0
    for i in 0..=n {
        let th = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (i as f32) / (n as f32);
        verts.push([th.cos() * r, 0.0, th.sin() * r]); // ring = index 1+i
    }
    // 侧面（弧面）：外法线朝 +径向
    for i in 0..n {
        indices.extend_from_slice(&[1 + i as u32, 0, 2 + i as u32]);
    }
    // 平背面（直径端点 + 顶点）：法线朝 -X
    indices.extend_from_slice(&[1, 1 + n as u32, 0]);
    // 底面扇形：法线朝 -Y
    for i in 1..n {
        indices.extend_from_slice(&[1, 1 + i as u32, 2 + i as u32]);
    }
    build_mesh(&verts, &indices)
}
pub fn ball(r: f32) -> Mesh {
    Sphere::new(r).mesh().ico(2).unwrap()
}
pub fn slab(w: f32, h: f32) -> Mesh {
    Cuboid::new(w, h, 0.02).mesh().build()
}

// ── 异形砖 Primitives ──────────────────────────────────────────────────────
/// 楔形砖（拱心石/拱券专用）：沿 X 方向收窄
/// w_bottom = 底面宽（拱的外侧），w_top = 顶面宽（拱的内侧/拱心方向）
/// height = 砖高（径向，层厚），depth = 墙厚方向
pub fn arch_wedge(w_bottom: f32, w_top: f32, height: f32, depth: f32) -> Mesh {
    let wb2 = w_bottom * 0.5;
    let wt2 = w_top    * 0.5;
    let h2  = height   * 0.5;
    let d2  = depth    * 0.5;
    let verts = vec![
        // 底面（Y=-h2）
        [-wb2, -h2, -d2], [ wb2, -h2, -d2], [ wb2, -h2, d2], [-wb2, -h2, d2],
        // 顶面（Y=+h2）
        [-wt2,  h2, -d2], [ wt2,  h2, -d2], [ wt2,  h2, d2], [-wt2,  h2, d2],
    ];
    let indices = vec![
        0,1,2, 0,2,3,   // 底
        4,6,5, 4,7,6,   // 顶
        0,4,5, 0,5,1,   // 前
        2,6,7, 2,7,3,   // 后
        0,3,7, 0,7,4,   // 左
        1,5,6, 1,6,2,   // 右
    ];
    build_mesh(&verts, &indices)
}

/// 径向楔形砖（圆形塔楼/鼓座专用）：沿 Z 方向（径向）收窄
/// 内弧面（Z=-d2）窄 = w_inner，外弧面（Z=+d2）宽 = w_outer
/// height = 层厚，depth = 墙厚（径向长度）
pub fn radial_wedge(w_outer: f32, w_inner: f32, height: f32, depth: f32) -> Mesh {
    let wbi = w_inner * 0.5;
    let wbo = w_outer * 0.5;
    let h2  = height  * 0.5;
    let d2  = depth   * 0.5;
    let verts = vec![
        // 内面（Z = -d2，朝圆心方向，外法线 -Z）
        [-wbi, -h2, -d2], [ wbi, -h2, -d2], [ wbi, h2, -d2], [-wbi, h2, -d2],
        // 外面（Z = +d2，朝外，外法线 +Z）
        [-wbo, -h2,  d2], [ wbo, -h2,  d2], [ wbo, h2,  d2], [-wbo, h2,  d2],
    ];
    // 所有三角形法线都按"封闭多面体外法线"方向写的绕序（右手坐标系）
    let indices = vec![
        // 内面：外法线朝 -Z（从圆心看向外）
        0,2,1, 0,3,2,
        // 外面：外法线朝 +Z
        4,5,6, 4,6,7,
        // 底面（Y=-h2）：外法线朝 -Y
        4,0,1, 4,1,5,
        // 顶面（Y=+h2）：外法线朝 +Y
        3,7,6, 3,6,2,
        // 左面（小 X 方向）：外法线朝 -X
        0,4,7, 0,7,3,
        // 右面（大 X 方向）：外法线朝 +X
        1,2,6, 1,6,5,
    ];
    build_mesh(&verts, &indices)
}

/// 四面体三角砖（直角）：用于转角缝隙填充、破损装饰
/// 底面在 Y=0 平面（X×Z 两直角边），顶点在 (0,b,0)
pub fn tetra_brick(a: f32, b: f32, c: f32) -> Mesh {
    let verts = vec![
        [0.0, 0.0, 0.0],
        [a,   0.0, 0.0],
        [0.0, 0.0, c  ],
        [0.0, b,   0.0],
    ];
    let indices = vec![
        0,2,1,  // 底
        0,1,3,  // 正
        0,3,2,  // 左
        1,2,3,  // 斜
    ];
    build_mesh(&verts, &indices)
}

/// 八角柱砖：装饰性柱础、柱头、独立石柱
pub fn octagon_prism(radius: f32, height: f32) -> Mesh {
    // 顶点 = 底面8个（偶索引）+ 顶面8个（奇索引）
    let mut verts = Vec::with_capacity(16);
    let mut indices = Vec::with_capacity(8 * 2 * 3 + 8 * 2 * 3); // 顶+底三角扇 + 侧面8组
    let h2 = height * 0.5;
    for i in 0..8 {
        let th = (i as f32 / 8.0) * std::f32::consts::TAU;
        let x = th.cos() * radius;
        let z = th.sin() * radius;
        verts.push([x, -h2, z]); // 底
        verts.push([x,  h2, z]); // 顶
    }
    // 顶面：三角扇（以顶点1为锚点顺时针）
    for i in 2..8 {
        indices.extend(&[1u32, (i as u32) * 2 + 1, (i as u32 - 1) * 2 + 1]);
    }
    // 底面：三角扇，法线反转（以顶点0为锚点逆时针）
    for i in 2..8 {
        indices.extend(&[0u32, (i as u32 - 1) * 2, (i as u32) * 2]);
    }
    // 侧面：8 个四边形（每个2个三角）
    for i in 0..8 {
        let a = (i as u32) * 2;
        let b = (((i + 1) % 8) as u32) * 2;
        indices.extend(&[a, b, b + 1, a, b + 1, a + 1]);
    }
    build_mesh(&verts, &indices)
}

// ── 内部：从顶点+索引构建 Mesh（补齐 NORMAL+UV_0 使与 Cuboid 合批兼容）──────────
fn build_mesh(verts: &[[f32; 3]], indices: &[u32]) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let n = verts.len();
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts.to_vec());
    // placeholder normals/UVs —— merged_flat 里 compute_flat_normals 会统一重算法线；
    // 这里必须填充相同数量的元素，否则 Mesh::merge 时因属性不匹配而丢顶点。
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0,   vec![[0.0f32, 0.0];     n]);
    m.insert_indices(Indices::U32(indices.to_vec()));
    m
}

// ═══════════════════════════════════════════════════════════════════════════
// 砌墙工具（chapel + basilica 共用）
// ═══════════════════════════════════════════════════════════════════════════

/// Small deterministic pseudo-random in [0,1) from an integer key (reproducible layouts).
pub fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x1f1f1f1f) ^ ((y as u32) << 7);
    h = h.wrapping_mul(0x1f1f1f1f);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

pub struct StoneCols {
    pub light: [f32; 4],
    pub mid:   [f32; 4],
    pub dark:  [f32; 4],
}
pub fn stone_cols() -> StoneCols {
    StoneCols {
        light: lin(STONE_LIGHT),
        mid:   lin(STONE),
        dark:  lin(STONE_DARK),
    }
}
pub fn pick(c: &StoneCols, x: i32, y: i32) -> [f32; 4] {
    let r = hash2(x, y * 31 + 7);
    if r < 0.18 { c.dark } else if r < 0.55 { c.mid } else { c.light }
}

/// Opening (rectangular void) inside a wall: along-axis range + height range.
/// along ∈ [0, len] is the offset measured along the wall direction from base.
/// y_low, y_high are world-space heights (measured from floor y=0).
#[derive(Clone, Copy)]
pub struct WallVoid {
    pub along_lo: f32,
    pub along_hi: f32,
    pub y_lo:     f32,
    pub y_hi:     f32,
}

/// Random-rubble (or intact, with drop=0) coursed wall.  Blocks have non-uniform
/// width/thickness with slight rotation jitter.  Course heights are *fixed* so
/// the top course aligns exactly with wall_h.
///
/// `voids` are strict AABB-intersection checks (no block corner pokes into an
/// opening).  `skip_along_ranges` are exclusive along-axis ranges to completely
/// skip — useful for avoiding double-draw where a neighbour wall shares the
/// same plane (e.g. a tower sharing a façade with the nave).
pub fn rubble_wall(
    along: char,
    base_x: f32,
    base_z: f32,
    len: f32,
    wall_h: f32,
    drop: f32,
    cols: &StoneCols,
    voids: &[WallVoid],
    parts: &mut Vec<(Mesh, [f32; 4])>,
    skip_along_ranges: &[(f32, f32)],
    block_h: f32,       // nominal course height (e.g. 0.5)
    wall_t: f32,        // wall thickness (e.g. 0.8 / 1.0)
    block_w_nom: f32,   // nominal block width (e.g. 1.0)
) {
    let courses = (wall_h / block_h).ceil() as i32;
    for cy in 0..courses {
        // fixed course layout so top course lands exactly at wall_h
        let course_bottom = cy as f32 * block_h;
        let course_top    = (cy + 1) as f32 * block_h;
        if course_bottom >= wall_h + 0.01 { break; }
        let course_h = course_top - course_bottom;

        // running bond: alternate course starts offset half a nominal block
        let mut along_pos = if cy % 2 == 0 { 0.0 } else { block_w_nom * 0.35 };

        while along_pos < len {
            let seed_w = cy * 101 + along_pos as i32;
            let bw = block_w_nom * (0.75 + hash2(seed_w, seed_w * 13) * 0.5); // ± 25%
            let bh = course_h   * (0.85 + hash2(seed_w + 1, cy)     * 0.12); // 85–97 %
            let bd = wall_t     * (0.82 + hash2(seed_w + 2, cy * 5) * 0.30);
            if along_pos + bw > len + 0.01 { break; }
            let block_along_lo = along_pos;
            let block_along_hi = along_pos + bw;

            // skip user-supplied ranges (tower shares, etc.)
            let mut skip_range = false;
            for (slo, shi) in skip_along_ranges {
                if block_along_hi > *slo && block_along_lo < *shi { skip_range = true; break; }
            }
            if skip_range { along_pos += bw * 0.94; continue; }

            // void intersection — STRICT AABB
            let block_y_lo = course_bottom + (course_h - bh) * 0.5;
            let block_y_hi = block_y_lo + bh;
            let mut in_void = false;
            for v in voids {
                let along_ok = block_along_hi > v.along_lo && block_along_lo < v.along_hi;
                let y_ok     = block_y_hi     > v.y_lo     && block_y_lo     < v.y_hi;
                if along_ok && y_ok { in_void = true; break; }
            }
            if in_void { along_pos += bw * 0.94; continue; }

            // ruin drop (drop = 0 → intact building)
            let top_frac = course_bottom / wall_h.max(0.01);
            let r = hash2(seed_w, cy * 31 + 1);
            let miss_top = drop > 0.0 && r < 0.5 && top_frac > 1.0 - drop;
            let miss_rand = drop > 0.0 && r < 0.04;
            if miss_top || miss_rand { along_pos += bw * 0.94; continue; }

            // place
            let block_along_centre = along_pos + bw * 0.5;
            let course_mid_y = course_bottom + course_h * 0.5;
            let (px, pz) = match along {
                'x' => (base_x, base_z + block_along_centre),
                _   => (base_x + block_along_centre, base_z),
            };
            let mut m = match along {
                'x' => cuboid(bd, bh * 0.97, bw * 0.95),
                _   => cuboid(bw * 0.95, bh * 0.97, bd),
            };
            // ±1.8° rotation (slight uncoursed feel but tidier for an intact basilica)
            let jitter = (hash2(seed_w + 3, cy * 17) - 0.5) * 0.032;
            m = m.rotated_by(Quat::from_rotation_y(jitter));
            m = m.translated_by(Vec3::new(px, course_mid_y, pz));
            parts.push((m, pick(cols, cy, seed_w)));

            along_pos += bw * 0.94;
        }
    }
}

/// Convenience: merge a `(mesh, vertex-colour)` batch via merged_flat.
pub fn mesh_from_parts(parts: Vec<(Mesh, [f32; 4])>) -> Mesh {
    merged_flat(parts.into_iter().map(|(m, c)| tinted(m, c)).collect())
}