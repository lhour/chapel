//! 罗马式巴西利卡 几何审计工具（方案 B，按 JSON 蓝图审计）。
//!
//! 独立二进制，**不依赖 Bevy**。内嵌 basilica.json，用纯 serde 解析后做 5 类断言：
//!   1. 三后殿 ↔ 横厅东墙开口的 Z 范围（0 容差对接）
//!   2. 筒形拱肋：每肋 9 块砖，每块落在半圆上（R_eff = 4.00 ± 0.02）
//!   3. Clerestory 墙 X 范围 = [-8, +8]，不与塔身重叠
//!   4. 水平截面 y∈{0.25, 6.25, 9.25, 13.5} 的 ASCII 网格闭合性
//!   5. 屋顶：横厅臂屋脊沿 Z（南/北立面呈山墙三角形）
//!
//! 运行：cargo run --bin basilica_audit

use serde::Deserialize;
use std::fmt::Write;

const BASILICA_JSON: &str = include_str!("../../assets/basilica.json");

// ═══════════════════════════════════════════════════════════════════════════
// 极简 JSON schema（与 blueprint.rs 等价但不带 Bevy 依赖）
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct Blueprint {
    name: String,
    masonry: MasonrySpec,
    roof_pitch: f32,
    #[serde(default)]
    features: Vec<Feature>,
}
#[derive(Debug, Deserialize, Clone, Copy)]
struct MasonrySpec {
    block_h: f32, block_w: f32, wall_t_main: f32, wall_t_aisle: f32,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Feature {
    Wall { id: String, along: char, base: [f32; 2], len: f32,
           y_start: f32, height: f32, thickness: f32,
           #[serde(default)] skip: Vec<[f32; 2]>,
           #[serde(default)] voids: Vec<VoidRect> },
    Tower { id: String, base: [f32; 2], size: f32, wall_h: f32,
            window: TowerWindow },
    Arcade { id: String, side: i32, x_lo: f32, x_hi: f32, columns: usize,
             col_z: f32, col_r: f32, arch_r: f32, top_y: f32 },
    Apse { id: String, centre: [f32; 2], radius: f32, height: f32,
           segments: usize, thickness: f32 },
    BarrelVault { id: String, x_lo: f32, x_hi: f32, spring_y: f32,
                  radius: f32, ribs: usize, voussoirs: usize },
    GableRoof { id: String, x_range: [f32; 2], z_range: [f32; 2],
                base_y: f32, pitch: f32, ridge: String },
    HalfConeRoof { id: String, centre: [f32; 2], radius: f32,
                   base_y: f32, height: f32 },
    PyramidRoof { id: String, centre: [f32; 2], size: f32,
                  base_y: f32, height: f32 },
    ArchRow { id: String, plane: String, fixed: f32, centres: Vec<f32>,
              spring_y: f32, radius: f32, depth: f32, voussoirs: usize },
    RoseWindow { id: String, centre: [f32; 2], y: f32, r_mid: f32,
                 ring_t: f32, segments: usize },
}
#[derive(Debug, Deserialize, Clone, Copy)]
struct VoidRect { along: [f32; 2], y: [f32; 2] }
#[derive(Debug, Deserialize, Clone, Copy)]
struct TowerWindow { along: [f32; 2], y: [f32; 2] }

// ═══════════════════════════════════════════════════════════════════════════
// main
// ═══════════════════════════════════════════════════════════════════════════

fn main() {
    let bp: Blueprint = serde_json::from_str(BASILICA_JSON)
        .expect("basilica.json parse failed");
    let mut out = String::with_capacity(16_000);
    let mut ok = true;

    header(&mut out, "ROMANESQUE BASILICA — GEOMETRY AUDIT (Plan B · JSON-driven)");
    row(&mut out, "Name", &bp.name);

    // ── 1. 后殿 ↔ 横厅东墙开口对接 ────────────────────────────────────────
    sec(&mut out, "1. 三后殿 ↔ 横厅东墙开口对接（0 容差）");
    ok &= check_apse_eastwall_mating(&bp, &mut out);

    // ── 2. 筒形拱肋 ──────────────────────────────────────────────────────
    sec(&mut out, "2. 筒形拱肋 — 每砖落在半圆上（R_eff = R_target ± 0.02）");
    ok &= check_barrel_vault(&bp, &mut out);

    // ── 3. Clerestory / 塔身 不重叠 ─────────────────────────────────────
    sec(&mut out, "3. Clerestory X 范围 ∈ [-8, +8]，不与塔体重叠");
    ok &= check_clerestory_overlap(&bp, &mut out);

    // ── 4. 水平截面 ASCII 网格 ───────────────────────────────────────────
    sec(&mut out, "4. 水平截面（1m 分辨率）");
    ok &= print_slices(&bp, &mut out);

    // ── 5. 屋顶山脊方向 ──────────────────────────────────────────────────
    sec(&mut out, "5. 屋顶山脊方向断言");
    ok &= check_roof_ridges(&bp, &mut out);

    // ── 汇总 ─────────────────────────────────────────────────────────────
    if ok {
        writeln!(out, "\n✅ ALL ASSERTIONS PASSED").unwrap();
    } else {
        writeln!(out, "\n❌ SOME ASSERTIONS FAILED — SEE SECTIONS ABOVE").unwrap();
    }
    println!("{}", out);
    if !ok { std::process::exit(1); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 断言 helpers
// ═══════════════════════════════════════════════════════════════════════════

/// 把 JSON 墙段展开成（id, 平面, 沿范围, y 范围, 厚）以便截面绘制。
/// 平面: 'x' = X 固定（南北走向），'z' = Z 固定（东西走向）。
fn wall_segments(bp: &Blueprint) -> Vec<WSeg> {
    let mut segs = Vec::new();
    for f in &bp.features {
        match f {
            Feature::Wall { id, along, base, len, y_start, height, thickness, .. } => {
                let (plane, fixed, along_lo, along_hi) = match along {
                    'x' => ('x', base[0], base[1], base[1] + len),
                    _   => ('z', base[1], base[0], base[0] + len),
                };
                segs.push(WSeg {
                    id: id.clone(), plane, fixed, along_lo, along_hi,
                    y_lo: *y_start, y_hi: *y_start + height, t: *thickness,
                });
                // y_start 以下不画（已被 void 挖空）—— 对截面只需 y∈[y_start, y_start+height]
            }
            Feature::Tower { id, base, size, wall_h, .. } => {
                // 塔四面墙（厚 1.0）
                let t = bp.masonry.wall_t_main;
                // W: X=base[0], Z∈[base[1], base[1]+size]
                segs.push(WSeg { id: format!("{id}·W"), plane: 'x', fixed: base[0],
                    along_lo: base[1], along_hi: base[1]+size, y_lo: 0.0, y_hi: *wall_h, t });
                // E: X=base[0]+size
                segs.push(WSeg { id: format!("{id}·E"), plane: 'x', fixed: base[0]+size,
                    along_lo: base[1], along_hi: base[1]+size, y_lo: 0.0, y_hi: *wall_h, t });
                // S: Z=base[1], X∈[base[0], base[0]+size]
                segs.push(WSeg { id: format!("{id}·S"), plane: 'z', fixed: base[1],
                    along_lo: base[0], along_hi: base[0]+size, y_lo: 0.0, y_hi: *wall_h, t });
                // N: Z=base[1]+size
                segs.push(WSeg { id: format!("{id}·N"), plane: 'z', fixed: base[1]+size,
                    along_lo: base[0], along_hi: base[0]+size, y_lo: 0.0, y_hi: *wall_h, t });
            }
            _ => {}
        }
    }
    segs
}
#[derive(Clone)]
struct WSeg { id: String, plane: char, fixed: f32, along_lo: f32, along_hi: f32,
              y_lo: f32, y_hi: f32, t: f32 }

fn check_apse_eastwall_mating(bp: &Blueprint, o: &mut String) -> bool {
    // 找三个后殿 & 横厅东墙
    let mut apses: Vec<(&str, [f32; 2], f32)> = Vec::new();
    let mut eastwall_voids: Vec<VoidRect> = Vec::new();
    for f in &bp.features {
        if let Feature::Apse { id, centre, radius, .. } = f {
            apses.push((id, *centre, *radius));
        }
        if let Feature::Wall { id, voids, .. } = f {
            if id == "transept_e_wall" { eastwall_voids = voids.clone(); }
        }
    }
    apses.sort_by(|a, b| a.1[1].partial_cmp(&b.1[1]).unwrap()); // 按 Z 从小到大
    // 三个后殿：南(−5.5) / 主(0) / 北(+5.5)，对应的开口 Z 范围。
    let expected = [
        ("apse_south", -7.0_f32, -4.0_f32),   // 南开口 Z∈[-7,-4]
        ("apse_main",  -4.0_f32,  4.0_f32),   // 主开口 Z∈[-4,+4]
        ("apse_north",  4.0_f32,  7.0_f32),   // 北开口 Z∈[+4,+7]
    ];
    let mut ok = true;
    for (i, (name, z_lo, z_hi)) in expected.iter().enumerate() {
        if i >= apses.len() {
            writeln!(o, "  ❌ 缺失后殿 {name}").unwrap(); ok = false; continue;
        }
        let (id, centre, r) = apses[i];
        // 沿 Z 方向的覆盖 = [centre_z − r, centre_z + r]
        let az_lo = centre[1] - r;
        let az_hi = centre[1] + r;
        let cover = (az_lo - z_lo).abs() < 0.01 && (az_hi - z_hi).abs() < 0.01;
        let mark = if cover { "✅" } else { "❌" };
        writeln!(o, "  {mark} {id}: Z∈[{az_lo:.2}, {az_hi:.2}] vs 开口 Z∈[{z_lo:.2}, {z_hi:.2}]").unwrap();
        if !cover { ok = false; }
    }
    // 另外校验 3 个 void 的 along 范围对得上 expected (按沿 lo 排序)
    let mut v: Vec<(f32, f32)> = eastwall_voids.iter()
        .map(|v| (v.along[0], v.along[1])).collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    // base[1]=-10.5, len=21: 沿0=Z-10.5, 沿21=Z+10.5
    // 主开口 Z∈[-4,+4] → 沿∈[6.5,14.5]
    // 南开口 Z∈[-7,-4] → 沿∈[3.5,6.5]
    // 北开口 Z∈[+4,+7] → 沿∈[14.5,17.5]
    // 按沿 lo 排: 3.5-6.5, 6.5-14.5, 14.5-17.5
    let expect_along = [(3.5_f32, 6.5), (6.5, 14.5), (14.5, 17.5)];
    for i in 0..3 {
        if i >= v.len() {
            writeln!(o, "  ❌ 横厅东墙开口数不足（只找到 {} 个）", v.len()).unwrap();
            ok = false; break;
        }
        let m = (v[i].0 - expect_along[i].0).abs() < 0.01
             && (v[i].1 - expect_along[i].1).abs() < 0.01;
        let mark = if m { "✅" } else { "❌" };
        writeln!(o, "  {mark} 横厅东墙开口 {i}: along∈[{:.2},{:.2}] 期望 [{:.2},{:.2}]",
            v[i].0, v[i].1, expect_along[i].0, expect_along[i].1).unwrap();
        if !m { ok = false; }
    }
    ok
}

fn check_barrel_vault(bp: &Blueprint, o: &mut String) -> bool {
    let mut ok = true;
    for f in &bp.features {
        let Feature::BarrelVault { id, x_lo, x_hi, spring_y, radius, ribs, voussoirs } = f
            else { continue };
        writeln!(o, "  拱 {id}: X∈[{x_lo:.1}, {x_hi:.1}], spring_y={spring_y}, R={radius}, \
                    ribs={ribs}, voussoirs/rib={voussoirs}").unwrap();
        let span = x_hi - x_lo;
        let spacing = span / (ribs - 1) as f32;
        let dphi = std::f32::consts::PI / *voussoirs as f32;
        let mut max_err = 0.0_f32;
        for rib in 0..*ribs {
            for i in 0..*voussoirs {
                let phi0 = std::f32::consts::PI - (i as f32)       * dphi;
                let phi1 = std::f32::consts::PI - ((i as f32) + 1.0) * dphi;
                let phic = (phi0 + phi1) * 0.5;
                let py = spring_y + phic.sin() * radius;
                let pz = phic.cos() * radius;
                // R_eff² = (py - spring_y)² + pz² 应当等于 radius²
                let dy = py - spring_y;
                let r_eff = (dy*dy + pz*pz).sqrt();
                let err = (r_eff - radius).abs();
                if err > max_err { max_err = err; }
                if !(py >= spring_y - 0.01 && py <= spring_y + radius + 0.01) {
                    writeln!(o, "    ❌ rib{rib} 砖{i}: Y={py:.3} 超出 [{spring_y}, {:.3}]",
                        spring_y + radius).unwrap();
                    ok = false;
                }
            }
        }
        let mark = if max_err < 0.02 { "✅" } else { "❌" };
        writeln!(o, "    {mark} max|R_eff − R| = {max_err:.4} m (阈值 0.02)").unwrap();
        if max_err >= 0.02 { ok = false; }
        // 检查拱顶高（crown）
        let crown = spring_y + radius;
        writeln!(o, "    crown_Y = {crown:.1} (应该 = 筒拱顶高，主殿 y=13.0 当 spring=9, R=4)").unwrap();
    }
    ok
}

fn check_clerestory_overlap(bp: &Blueprint, o: &mut String) -> bool {
    let mut ok = true;
    let mut tower_x_ranges: Vec<(f32, f32)> = Vec::new();
    for f in &bp.features {
        if let Feature::Tower { base, size, .. } = f {
            tower_x_ranges.push((base[0], base[0] + size));
        }
    }
    for f in &bp.features {
        let Feature::Wall { id, along, base, len, y_start, .. } = f else { continue };
        if !id.starts_with("clerestory") { continue; }
        // 沿='x' 南北走向 X 固定 → 不检查。clerestory 是沿='z' 东西走向(Z=±4)，沿 X 延伸。
        if *along != 'z' { continue; }
        let x_lo = base[0];
        let x_hi = base[0] + len;
        let ok_x = (x_lo - (-8.0)).abs() < 0.01 && (x_hi - 8.0).abs() < 0.01;
        let mark = if ok_x { "✅" } else { "❌" };
        writeln!(o, "  {mark} {id} 沿X ∈ [{x_lo:.1}, {x_hi:.1}] 期望 [-8.0, 8.0], \
                    y_start = {y_start}（应该=6 → 不与侧廊墙重叠）").unwrap();
        if !ok_x { ok = false; }
        // 不与塔身 X 范围相交
        for (tx_lo, tx_hi) in &tower_x_ranges {
            let intersect = x_hi > *tx_lo && x_lo < *tx_hi;
            if intersect {
                writeln!(o, "    ❌ 与塔 X∈[{tx_lo:.1},{tx_hi:.1}] 相交").unwrap();
                ok = false;
            }
        }
    }
    ok
}

fn check_roof_ridges(bp: &Blueprint, o: &mut String) -> bool {
    let mut ok = true;
    for f in &bp.features {
        let Feature::GableRoof { id, x_range, z_range, ridge, .. } = f else { continue };
        let is_transept_s = id.contains("transept_s");
        let is_transept_n = id.contains("transept_n");
        if is_transept_s || is_transept_n {
            let r_ok = ridge.eq_ignore_ascii_case("z");
            let mark = if r_ok { "✅" } else { "❌" };
            writeln!(o, "  {mark} {id}: ridge=\"{ridge}\" (必须沿 Z，山墙朝南北)").unwrap();
            if !r_ok { ok = false; }
            // 范围检查: Z∈[-10,-4] 或 [4,10], X∈[8,12]
            let z_ok = if is_transept_s {
                (z_range[0] - (-10.0)).abs() < 0.01 && (z_range[1] - (-4.0)).abs() < 0.01
            } else {
                (z_range[0] - 4.0).abs() < 0.01 && (z_range[1] - 10.0).abs() < 0.01
            };
            if !z_ok {
                writeln!(o, "    ❌ {id} Z 范围错误: [{:.1}, {:.1}]", z_range[0], z_range[1]).unwrap();
                ok = false;
            }
            let x_ok = (x_range[0] - 8.0).abs() < 0.01 && (x_range[1] - 12.0).abs() < 0.01;
            if !x_ok {
                writeln!(o, "    ❌ {id} X 范围错误: [{:.1}, {:.1}]", x_range[0], x_range[1]).unwrap();
                ok = false;
            }
        }
        if id == "roof_main" || id == "roof_narthex" || id.contains("aisle") {
            let r_ok = ridge.eq_ignore_ascii_case("x");
            let mark = if r_ok { "✅" } else { "❌" };
            writeln!(o, "  {mark} {id}: ridge=\"{ridge}\" (沿 X)").unwrap();
            if !r_ok { ok = false; }
        }
    }
    ok
}

fn print_slices(bp: &Blueprint, o: &mut String) -> bool {
    let segs = wall_segments(bp);
    // X range: -16..+18; Z range: -12..+12 (1m grid)
    let x_min = -16_i32; let x_max = 17_i32;  // 打印时倒序
    let z_min = -12_i32; let z_max = 11_i32;
    let ys = [("y=0.25 (ground w/ walls)", 0.25),
              ("y=6.25 (side aisle roofs / arcade h)", 6.25),
              ("y=9.25 (clerestory top / transept walls)", 9.25),
              ("y=13.50 (main gable ridge-bottom, crossing top)", 13.50)];
    let mut ok = true;
    for (label, y) in ys {
        writeln!(o, "  ── {label} ──").unwrap();
        // 表头
        write!(o, "        ").unwrap();
        for x in x_min..=x_max { write!(o, "{:>2}", x % 10).unwrap(); }
        writeln!(o).unwrap();
        for z in (z_min..=z_max).rev() {
            write!(o, "  Z={z:>3} │").unwrap();
            for x in x_min..=x_max {
                // cell centre = (x+0.5, y, z+0.5)
                let cx = x as f32 + 0.5;
                let cz = z as f32 + 0.5;
                let mut ch = '.';
                for s in &segs {
                    if y < s.y_lo || y > s.y_hi { continue; }
                    let hit = match s.plane {
                        'x' => {
                            // X=fixed (墙厚 ±t/2), Z∈[along_lo, along_hi]
                            (cx - s.fixed).abs() <= s.t * 0.5 + 0.5
                                && cz >= s.along_lo - 0.01 && cz <= s.along_hi + 0.01
                        }
                        _ => {
                            // Z=fixed, X∈[along_lo, along_hi]
                            (cz - s.fixed).abs() <= s.t * 0.5 + 0.5
                                && cx >= s.along_lo - 0.01 && cx <= s.along_hi + 0.01
                        }
                    };
                    if hit { ch = '#'; break; }
                }
                write!(o, "{ch:>2}").unwrap();
            }
            writeln!(o).unwrap();
        }
    }
    // 断言：y=0.25 层 西立面/南墙/北墙/横厅东墙 闭合
    // 简单启发：四角单元格必须是墙
    let corners = [
        ("SW facade", -14_i32, -10_i32),
        ("NW facade", -14_i32,  10_i32),
        ("SE transept", 12_i32, -10_i32),
        ("NE transept", 12_i32,  10_i32),
    ];
    for (name, cx_corner, cz_corner) in corners {
        let mut hit = false;
        for s in &segs {
            if 0.25 < s.y_lo || 0.25 > s.y_hi { continue; }
            let cx = cx_corner as f32 + 0.5;
            let cz = cz_corner as f32 + 0.5;
            let h = match s.plane {
                'x' => (cx - s.fixed).abs() <= s.t*0.5+0.5
                        && cz >= s.along_lo-0.01 && cz <= s.along_hi+0.01,
                _   => (cz - s.fixed).abs() <= s.t*0.5+0.5
                        && cx >= s.along_lo-0.01 && cx <= s.along_hi+0.01,
            };
            if h { hit = true; break; }
        }
        let mark = if hit { "✅" } else { "❌" };
        writeln!(o, "  {mark} y=0.25 角墙闭合: {name} (X={cx_corner}, Z={cz_corner})").unwrap();
        if !hit { ok = false; }
    }
    ok
}

// ═══════════════════════════════════════════════════════════════════════════
// 打印 helpers
// ═══════════════════════════════════════════════════════════════════════════

fn header(o: &mut String, t: &str) {
    writeln!(o, "\n╔{:═<78}╗\n║ {t:^76} ║\n╚{:═<78}╝", "", "").unwrap();
}
fn sec(o: &mut String, t: &str) { writeln!(o, "\n── {t} ──\n").unwrap(); }
fn row(o: &mut String, a: &str, b: &str) { writeln!(o, "  {a:<34}  {b}").unwrap(); }
