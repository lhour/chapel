//! 方案 B 核心：参数化 JSON 蓝图 + Dispatcher。
//!
//! JSON 是唯一真源（assets/basilica.json）。蓝图数据结构在此定义，
//! Dispatcher 把每个 Feature 翻译成 basilica.rs 中的砖砌原语调用。
//!
//! 加载方式：include_str!("assets/basilica.json") 编译期内嵌，
//! 避免运行时文件路径依赖（审计工具 basilica_audit 也用同一字符串解析）。

use bevy::prelude::*;
use bevy::mesh::Mesh;
use serde::Deserialize;

use crate::basilica::{self, Masonry, Parts};
use crate::geoms::{
    self, WallVoid, StoneCols,
    lin, ROOF, ROOF_LIGHT,
};

// ═══════════════════════════════════════════════════════════════════════════
// JSON Schema（与 assets/basilica.json 字段一一对应）
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct Blueprint {
    pub name: String,
    pub masonry: MasonrySpec,
    pub roof_pitch: f32,
    #[serde(default)]
    pub features: Vec<Feature>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct MasonrySpec {
    pub block_h: f32,
    pub block_w: f32,
    /// 主墙厚（西立面、横厅、山墙等承重主墙）
    pub wall_t_main: f32,
    /// 侧墙厚（侧廊、后殿等次要墙体）
    pub wall_t_aisle: f32,
}

/// 墙体内单个 AABB 开口（纯矩形，不包含圆拱台阶；圆拱台阶由 Dispatcher 用 round_top_void 展开）。
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct VoidRect {
    /// 沿走向范围 [lo, hi]
    pub along: [f32; 2],
    /// 高度范围 [lo, hi]
    pub y: [f32; 2],
}

/// 塔窗规格（沿+高，默认按圆拱顶展开成台阶）
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct TowerWindow {
    pub along: [f32; 2],
    pub y: [f32; 2],
}

// ── 每个 feature 的枚举体（按 type 字段 tag） ────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Feature {
    Wall {
        id: String,
        /// 'x' = 南北走向（X 固定），'z' = 东西走向（Z 固定）
        along: char,
        /// [base_x, base_z]
        base: [f32; 2],
        len: f32,
        y_start: f32,
        height: f32,
        thickness: f32,
        /// 沿走向跳过的区段（共享面防重绘），默认 []
        #[serde(default)]
        skip: Vec<[f32; 2]>,
        /// 墙内开口（AABB），默认 []
        #[serde(default)]
        voids: Vec<VoidRect>,
    },
    Tower {
        id: String,
        /// 塔西南角 [tx, tz]
        base: [f32; 2],
        size: f32,
        wall_h: f32,
        /// 圆拱窗范围（沿走向 + 高度）
        window: TowerWindow,
    },
    Arcade {
        id: String,
        /// -1 = 南侧廊，+1 = 北侧廊
        side: i32,
        x_lo: f32,
        x_hi: f32,
        columns: usize,
        col_z: f32,
        col_r: f32,
        arch_r: f32,
        top_y: f32,
    },
    Apse {
        id: String,
        centre: [f32; 2],
        radius: f32,
        height: f32,
        segments: usize,
        thickness: f32,
    },
    BarrelVault {
        id: String,
        x_lo: f32,
        x_hi: f32,
        spring_y: f32,
        radius: f32,
        ribs: usize,
        voussoirs: usize,
    },
    GableRoof {
        id: String,
        x_range: [f32; 2],
        z_range: [f32; 2],
        base_y: f32,
        pitch: f32,
        /// "x" / "z"
        ridge: String,
    },
    HalfConeRoof {
        id: String,
        centre: [f32; 2],
        radius: f32,
        base_y: f32,
        height: f32,
    },
    PyramidRoof {
        id: String,
        centre: [f32; 2],
        size: f32,
        base_y: f32,
        height: f32,
    },
    ArchRow {
        id: String,
        /// "xy" = 拱跨沿 X（XY 面，fixed = 墙 Z），"yz" = 拱跨沿 Z（YZ 面，fixed = 墙 X）
        plane: String,
        fixed: f32,
        centres: Vec<f32>,
        spring_y: f32,
        radius: f32,
        depth: f32,
        voussoirs: usize,
    },
    RoseWindow {
        id: String,
        /// [X 墙面, Z 圆心]
        centre: [f32; 2],
        y: f32,
        r_mid: f32,
        ring_t: f32,
        segments: usize,
    },
}

// ═══════════════════════════════════════════════════════════════════════════
// 载入 / 解析
// ═══════════════════════════════════════════════════════════════════════════

/// 解析 JSON 字符串为 Blueprint（带 panic 提示，便于开发期发现字段错误）。
pub fn parse(json: &str) -> Blueprint {
    match serde_json::from_str::<Blueprint>(json) {
        Ok(bp) => bp,
        Err(e) => {
            eprintln!("FATAL: basilica.json parse error: {e}");
            panic!("basilica.json invalid — {e}");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Dispatcher：把每个 Feature 翻译成 basilica.rs 原语调用。
// ═══════════════════════════════════════════════════════════════════════════

/// 一次性 dispatch 蓝图里的所有特征到 parts。
/// 返回：墙体用 Masonry 参数（供审计/外部分享）。
pub fn dispatch_all(
    bp: &Blueprint,
    cols: &StoneCols,
    parts: &mut Parts,
) -> Masonry {
    let masonry = Masonry {
        block_h: bp.masonry.block_h,
        block_w: bp.masonry.block_w,
    };
    let roof_dark  = lin(ROOF);
    let roof_light = lin(ROOF_LIGHT);
    let wt_main  = bp.masonry.wall_t_main;
    let wt_aisle = bp.masonry.wall_t_aisle;

    for f in &bp.features {
        dispatch_one(f, &masonry, wt_main, wt_aisle, cols, &roof_dark, &roof_light, parts);
    }
    masonry
}

pub fn dispatch_one(
    f: &Feature,
    masonry: &Masonry,
    wt_main: f32,
    wt_aisle: f32,
    cols: &StoneCols,
    roof_dark: &[f32; 4],
    roof_light: &[f32; 4],
    parts: &mut Parts,
) {
    match f {
        // ── 砖墙 ───────────────────────────────────────────────────────────
        Feature::Wall { along, base, len, y_start, height, thickness, skip, voids, .. } => {
            // VoidRect → WallVoid（当前 JSON 里都是纯 AABB）
            let wall_voids: Vec<WallVoid> = voids.iter().map(|v| WallVoid {
                along_lo: v.along[0],
                along_hi: v.along[1],
                y_lo:     v.y[0],
                y_hi:     v.y[1],
            }).collect();
            let skip_ranges: Vec<(f32, f32)> = skip.iter().map(|s| (s[0], s[1])).collect();
            basilica::wall(
                *along, base[0], base[1], *len, *y_start, *height, *thickness,
                cols, &wall_voids, &skip_ranges, masonry, parts,
            );
        }

        // ── 方塔 ───────────────────────────────────────────────────────────
        Feature::Tower { base, size, wall_h, window, .. } => {
            // 塔窗是圆拱顶：沿 [2.2, 3.8]（1.6 宽）, y [8.0, 11.5]
            // 半跨 r = 0.8，crown_y = spring + r = (11.5 - 0.8) + 0.8 = 11.5 ✓
            let win_voids = basilica::round_top_void(
                [window.along[0], window.along[1]],
                [window.y[0],     window.y[1]],
                masonry.block_h,
            );
            basilica::tower(
                *base, *size, *wall_h, &win_voids, wt_main, cols, masonry, parts,
            );
        }

        // ── 连拱廊 ─────────────────────────────────────────────────────────
        Feature::Arcade { side, x_lo, x_hi, columns, col_z, col_r, arch_r, top_y, .. } => {
            // arcade 墙厚取侧廊墙厚（与 aisle 外墙一致）
            basilica::arcade(
                *side, *x_lo, *x_hi, *columns, *col_z, *col_r, *arch_r, *top_y,
                wt_aisle, cols, masonry, parts,
            );
        }

        // ── 半圆后殿 ───────────────────────────────────────────────────────
        Feature::Apse { centre, radius, height, segments, thickness, .. } => {
            basilica::apse(
                *centre, *radius, *height, *thickness, *segments, cols, masonry, parts,
            );
        }

        // ── 筒形拱 ─────────────────────────────────────────────────────────
        Feature::BarrelVault { x_lo, x_hi, spring_y, radius, ribs, voussoirs, .. } => {
            basilica::barrel_vault(
                *x_lo, *x_hi, *spring_y, *radius, *ribs, *voussoirs, cols, parts,
            );
        }

        // ── 人字屋顶 ───────────────────────────────────────────────────────
        Feature::GableRoof { x_range, z_range, base_y, pitch, ridge, .. } => {
            let rch: char = ridge.chars().next().unwrap_or('x').to_ascii_lowercase();
            basilica::gable_roof(
                *x_range, *z_range, *base_y, *pitch, rch, roof_dark, roof_light, parts,
            );
        }

        // ── 后殿半圆锥顶 ───────────────────────────────────────────────────
        Feature::HalfConeRoof { centre, radius, base_y, height, .. } => {
            basilica::half_cone_roof(*centre, *radius, *base_y, *height, roof_dark, parts);
        }

        // ── 塔八棱锥台顶 ───────────────────────────────────────────────────
        Feature::PyramidRoof { centre, size, base_y, height, .. } => {
            basilica::pyramid_roof(*centre, *size, *base_y, *height, roof_dark, parts);
        }

        // ── 拱券排 ─────────────────────────────────────────────────────────
        Feature::ArchRow { plane, fixed, centres, spring_y, radius, depth, voussoirs, .. } => {
            // JSON plane: "xy" → 拱跨沿 X（XY 面，原语 plane='x'，fixed=Z）
            //             "yz" → 拱跨沿 Z（YZ 面，原语 plane='z'，fixed=X）
            let pl: char = if plane.eq_ignore_ascii_case("xy") { 'x' } else { 'z' };
            let cs: Vec<f32> = centres.clone();
            basilica::arch_row(
                pl, *fixed, &cs, *spring_y, *radius, *depth, *voussoirs, cols, parts,
            );
        }

        // ── 玫瑰窗砖环 ─────────────────────────────────────────────────────
        Feature::RoseWindow { centre, y, r_mid, ring_t, segments, .. } => {
            basilica::rose_window(
                *centre, *y, *r_mid, *ring_t, *segments, cols, parts,
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 便利：一次性合并 parts 成若干大 mesh（每 ~400 砖一块，避免单 mesh 过大）。
// ═══════════════════════════════════════════════════════════════════════════

pub fn merge_parts(parts: Parts) -> Vec<Mesh> {
    const CHUNK: usize = 400;
    let mut out = Vec::new();
    let mut cur: Vec<(Mesh, [f32; 4])> = Vec::with_capacity(CHUNK);
    for p in parts {
        cur.push(p);
        if cur.len() >= CHUNK {
            out.push(geoms::mesh(std::mem::take(&mut cur)));
            cur = Vec::with_capacity(CHUNK);
        }
    }
    if !cur.is_empty() {
        out.push(geoms::mesh(cur));
    }
    out
}
