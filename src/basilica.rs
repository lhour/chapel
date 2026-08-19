//! 砖砌建筑原语库（逐块 Cuboid / 楔形砖）。
//!
//! - 这是方案 B 的"施工队"层：Dispatcher 按 JSON 蓝图中的参数直接调用这里的函数。
//! - 每个函数只做两件事：逐块生成砖 mesh → push 到 (mesh, 颜色) 的 parts 列表。
//! - 所有坐标都是**世界坐标**（X=东西, Z=南北, Y=上下, 地面 y=0）。

use bevy::prelude::*;
use bevy::mesh::Mesh;

use crate::geoms::*;

// ═══════════════════════════════════════════════════════════════════════════
// 砌筑参数（蓝图传入，控制砖尺寸）
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
pub struct Masonry {
    pub block_h: f32,   // 层高（每皮砖高度基准）
    pub block_w: f32,   // 砖宽基准（乱毛石会在这个值附近抖动）
}

pub type Parts = Vec<(Mesh, [f32; 4])>;

// ── 辅助：把"带圆拱顶的矩形开口"展开成 rubble_wall 吃的 WallVoid 台阶列表 ──
/// 圆拱顶矩形：y[1] 是拱顶（crown），半跨 r = (along[1]−along[0])/2，
/// 起拱线 spring = y[1] − r。下部矩形 + 上部圆拱台阶。
pub fn round_top_void(along: [f32; 2], y: [f32; 2], bh: f32) -> Vec<WallVoid> {
    let mut out = Vec::new();
    let span = along[1] - along[0];
    let r = span * 0.5;
    let c = (along[0] + along[1]) * 0.5;
    let spring = y[1] - r;
    // 下部长方形（到起拱线略上方，防缝）
    out.push(WallVoid {
        along_lo: along[0], along_hi: along[1],
        y_lo: y[0], y_hi: spring + 0.02,
    });
    // 上部台阶带：每 bh 高一层，每层宽度按圆方程收缩
    let mut k = 0.0;
    while k * bh < r - 0.05 {
        let t0 = k * bh;
        let t1 = ((k + 1.0) * bh).min(r);
        let w = (r * r - t1 * t1).max(0.0).sqrt();
        if w > 0.06 {
            out.push(WallVoid {
                along_lo: c - w - 0.01, along_hi: c + w + 0.01,
                y_lo: spring + t0 - 0.01, y_hi: spring + t1 + 0.01,
            });
        }
        k += 1.0;
    }
    out
}

/// 圆形开口（玫瑰窗等）：按 bh 步长切成水平条带 AABB
pub fn circle_void(c: f32, cy: f32, r: f32, bh: f32) -> Vec<WallVoid> {
    let mut out = Vec::new();
    let bh = bh.max(0.25);
    let n = (r / bh).ceil() as i32;
    for j in -n..=n {
        let y0 = (cy + j as f32 * bh).max(cy - r);
        let y1 = (cy + (j + 1) as f32 * bh).min(cy + r);
        if y1 <= y0 { continue; }
        let d = (y0 - cy).abs().max((y1 - cy).abs());
        if d >= r { continue; }
        let w = (r * r - d * d).sqrt();
        if w > 0.06 {
            out.push(WallVoid {
                along_lo: c - w - 0.01, along_hi: c + w + 0.01,
                y_lo: y0 - 0.01, y_hi: y1 + 0.01,
            });
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 1 — 砖墙
// ═══════════════════════════════════════════════════════════════════════════
/// along = 'x' → 墙法线沿 X（南北走向墙，墙厚沿 X，砖沿 X 排列 → 不对：
///   rubble_wall 约定：along='x' 表示 base=(fixed_x, along_start_z)，
///   砖沿 Z 排列（沿 X 的墙=东西走向？不，读 geoms.rs rubble_wall 代码：
///   ```
///   let (px, pz) = match along {
///       'x' => (base_x, base_z + block_along_centre),
///       _   => (base_x + block_along_centre, base_z),
///   };
///   let mut m = match along {
///       'x' => cuboid(bd, bh*0.97, bw*0.95),   // X=厚, Z=砖宽
///       _   => cuboid(bw*0.95, bh*0.97, bd),   // X=砖宽, Z=厚
///   };
///   ```
///   结论：along='x' → 墙厚方向 = X（墙是 XZ 中 X=base_x 的一条，沿 Z 延伸 = 南北走向墙，比如西立面 X=-14）。
///         along='z' → 墙厚方向 = Z（墙是 Z=base_z 的一条，沿 X 延伸 = 东西走向墙，比如南墙 Z=-7）。

pub fn wall(
    along: char,          // 'x' = 南北走向(X固定), 'z' = 东西走向(Z固定)
    base_x: f32,          // 沿='x'时=固定X; 沿='z'时=起点X
    base_z: f32,          // 沿='x'时=起点Z; 沿='z'时=固定Z
    len: f32,             // 墙沿其走向的长度
    y_start: f32,         // 墙从多高开始砌（如 clerestory 从 6m 起）
    wall_h: f32,          // 墙总高度（砌到 y = wall_h）
    thickness: f32,       // 墙厚（沿法线方向）
    cols: &StoneCols,
    voids: &[WallVoid],
    skip_along_ranges: &[(f32, f32)],   // 沿走向跳过的区段（如塔+立面共享面）
    masonry: &Masonry,
    parts: &mut Parts,
) {
    // rubble_wall 只接受 y=0 起砌；y_start 以下用一幅大 void 挖空
    let mut v: Vec<WallVoid> = Vec::with_capacity(voids.len() + 1);
    if y_start > 0.01 {
        v.push(WallVoid { along_lo: 0.0, along_hi: len, y_lo: 0.0, y_hi: y_start });
    }
    v.extend_from_slice(voids);

    rubble_wall(
        along, base_x, base_z, len, wall_h, 0.0, cols, &v, parts,
        skip_along_ranges, masonry.block_h, thickness, masonry.block_w,
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 2 — 方塔（四面砖墙 + 圆拱窗 + 雉堞）
// ═══════════════════════════════════════════════════════════════════════════

pub fn tower(
    base: [f32; 2],       // 塔西南角 (tx, tz)
    size: f32,            // 塔平面边长
    wall_h: f32,          // 塔身墙高（不含雉堞）
    window_voids: &[WallVoid], // 带窗墙面的开口（圆拱窗已展开成台阶）
    thickness: f32,       // 塔墙厚
    cols: &StoneCols,
    masonry: &Masonry,
    parts: &mut Parts,
) {
    let (tx, tz) = (base[0], base[1]);
    let no_void: [WallVoid; 0] = [];

    // 四面墙：
    //   W: X=tx,        Z∈[tz, tz+size]      → along='x'
    //   E: X=tx+size,   Z∈[tz, tz+size]      → along='x'
    //   S: Z=tz,        X∈[tx, tx+size]      → along='z'（base=(tx, tz) 沿='z' 时沿X延伸, len=size ✓）
    //   N: Z=tz+size,   X∈[tx, tx+size]      → along='z'
    // 约定：W 面带窗（与西立面共享的那面），S/N 面带窗，E 面朝内一般无窗。
    wall('x', tx,        tz, size, 0.0, wall_h, thickness, cols, window_voids, &[], masonry, parts); // W
    wall('x', tx + size, tz, size, 0.0, wall_h, thickness, cols, &no_void,     &[], masonry, parts); // E
    wall('z', tx,        tz, size, 0.0, wall_h, thickness, cols, window_voids, &[], masonry, parts); // S
    wall('z', tx, tz + size, size, 0.0, wall_h, thickness, cols, window_voids, &[], masonry, parts); // N

    // 雉堞：1m 齿 + 1m 隙，每面 runs=(size/2) 个齿，四角深色
    let merlon = 1.0;
    let runs = (size / (merlon * 2.0)).floor() as i32;
    for face in 0..4 {
        for i in 0..runs {
            let ac = merlon + i as f32 * merlon * 2.0;
            let (along, fx, fz) = match face {
                0 => ('x', tx,        tz + ac),          // W 面齿: X=tx, Z 走向
                1 => ('x', tx + size, tz + ac),          // E
                2 => ('z', tx + ac,   tz),               // S 面齿: Z=tz, X 走向
                _ => ('z', tx + ac,   tz + size),        // N
            };
            let m = match along {
                'x' => cuboid(thickness * 0.95, 0.95, merlon * 0.95),
                _   => cuboid(merlon * 0.95, 0.95, thickness * 0.95),
            };
            let corner = i == 0 || i == runs - 1;
            parts.push((
                m.translated_by(Vec3::new(fx, wall_h + 0.5, fz)),
                if corner { cols.dark } else { pick(cols, face, i + 50) },
            ));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 3 — 半圆后殿（径向楔形砖，内弧窄外弧宽，与 chapel.rs 视觉验证版本一致）
// ═══════════════════════════════════════════════════════════════════════════

pub fn apse(
    centre: [f32; 2],     // 圆心 (cx, cz) — 直径端点在 Z=±r，弧朝 +X
    r_mid: f32,           // 中径（砖心所在圆）
    wall_h: f32,          // 墙高
    wall_t: f32,          // 墙厚（径向）
    segs: usize,          // 半圆周段数（8~10 视觉效果好）
    cols: &StoneCols,
    masonry: &Masonry,
    parts: &mut Parts,
) {
    let r_in  = r_mid - wall_t * 0.5;
    let r_out = r_mid + wall_t * 0.5;
    let dth   = std::f32::consts::PI / segs as f32; // 半圆周角
    let courses = (wall_h / masonry.block_h).ceil() as i32;
    let bh = wall_h / courses as f32; // 顶皮精确对齐 wall_h

    for seg in 0..segs {
        // seg 0 = 北端(θ=+π/2), seg segs-1 = 南端(θ=-π/2)
        let th0 =  std::f32::consts::FRAC_PI_2 - (seg as f32)       * dth;
        let th1 =  std::f32::consts::FRAC_PI_2 - ((seg as f32) + 1.0) * dth;
        let thc = (th0 + th1) * 0.5;

        let w_in  = r_in  * dth;
        let w_out = r_out * dth;
        let wedge_cx = centre[0] + thc.cos() * r_mid;
        let wedge_cz = centre[1] + thc.sin() * r_mid;
        // radial_wedge 局部 +Z(深度) 需指向外径向 = (cos thc, 0, sin thc)
        // Ry(α): +Z → (sin α, 0, cos α). 令 sin α = cos thc, cos α = sin thc → α = π/2 − thc
        let rot_angle = std::f32::consts::FRAC_PI_2 - thc;

        for cy in 0..courses {
            let y = cy as f32 * bh + bh * 0.5;
            let mut m = radial_wedge(w_out, w_in, bh * 0.9, wall_t * 0.9);
            m = m.rotated_by(Quat::from_rotation_y(rot_angle));
            m = m.translated_by(Vec3::new(wedge_cx, y, wedge_cz));
            parts.push((m, pick(cols, seg as i32, cy)));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 4 — 连拱廊（柱列 + 圆拱开口 + 拱券环，用于 nave 两侧）
// ═══════════════════════════════════════════════════════════════════════════

pub fn arcade(
    side: i32,            // -1 = 南侧廊（墙在 Z=−wall_z），+1 = 北侧廊
    x_lo: f32,
    x_hi: f32,
    n_columns: usize,
    col_z: f32,           // 柱列 Z 坐标（正值，side 决定符号）
    col_r: f32,           // 柱半径
    arch_r: f32,          // 拱半径 (= 半跨)
    top_y: f32,           // 拱顶到多高（= 侧廊屋顶底）
    wall_t: f32,
    cols: &StoneCols,
    masonry: &Masonry,
    parts: &mut Parts,
) {
    let wall_z = col_z * side as f32;
    let len = x_hi - x_lo;
    let spacing = len / (n_columns - 1) as f32;
    let spring_y = top_y - arch_r;
    let col_h = spring_y - 0.25;   // 柱身到起拱线以下 0.25（留 0.1 础 + 0.15 斗）

    // 1) 拱廊墙：每跨一个圆拱开口
    let mut voids: Vec<WallVoid> = Vec::new();
    for i in 0..(n_columns - 1) {
        let c = x_lo + (i as f32 + 0.5) * spacing;
        voids.extend(round_top_void(
            [c - arch_r, c + arch_r],
            [0.0, top_y + 0.02],
            masonry.block_h,
        ));
    }
    // wall along='z' → 沿 X 延伸（Z=wall_z 固定），base=(x_lo, wall_z)
    wall('z', x_lo, wall_z, len, 0.0, top_y, wall_t, cols, &voids, &[], masonry, parts);

    // 2) 柱（半露柱：柱心在墙面 Z=wall_z 上；础+身+斗三段）
    for i in 0..n_columns {
        let col_x = x_lo + i as f32 * spacing;
        // 础
        parts.push((cuboid(col_r * 2.2, 0.1, col_r * 2.2)
            .translated_by(Vec3::new(col_x, 0.05, wall_z)), cols.dark));
        // 身
        parts.push((octagon_prism(col_r, col_h)
            .translated_by(Vec3::new(col_x, col_h * 0.5 + 0.1, wall_z)), cols.light));
        // 斗
        parts.push((cuboid(col_r * 1.9, 0.15, col_r * 1.9)
            .translated_by(Vec3::new(col_x, col_h + 0.175, wall_z)), cols.dark));
    }

    // 3) 每跨一环拱券（从墙面突出来一点，视觉上嵌入开口）
    for i in 0..(n_columns - 1) {
        let c = x_lo + (i as f32 + 0.5) * spacing;
        // plane='x' = 拱跨沿 X（拱在 XY 面，fixed = 墙 Z）
        arch_row('x', wall_z, &[c], spring_y, arch_r, wall_t, 7, cols, parts);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 5 — 筒形拱（密排拱肋 = 连续砖砌圆柱壳，无 infill 阶梯假拱）
// ═══════════════════════════════════════════════════════════════════════════

pub fn barrel_vault(
    x_lo: f32,
    x_hi: f32,
    spring_y: f32,
    r_mid: f32,
    ribs: usize,
    voussoirs: usize,
    cols: &StoneCols,
    parts: &mut Parts,
) {
    let shell_t = 0.5; // 拱壳径向厚度
    let r_in = r_mid - shell_t * 0.5;
    let r_out = r_mid + shell_t * 0.5;
    let dr = r_out - r_in;
    let span = x_hi - x_lo;
    let spacing = span / (ribs - 1) as f32;
    let rib_depth = spacing * 1.02; // 密排：相邻肋轻微重叠 → 内表面连续
    let dphi = std::f32::consts::PI / voussoirs as f32;

    for rib in 0..ribs {
        let x = x_lo + rib as f32 * spacing;
        for i in 0..voussoirs {
            // phic ∈ [0, π]: 0→右拱脚(Z=+R), π/2→拱顶, π→左拱脚(Z=-R)
            // 这样 sin ≥ 0，拱肋只在上半圆，不会下半截埋进地面。
            let phi0 = std::f32::consts::PI - (i as f32)       * dphi;
            let phi1 = std::f32::consts::PI - ((i as f32) + 1.0) * dphi;
            let phic = (phi0 + phi1) * 0.5;

            let w_in  = (r_in  * dphi * 0.96).max(0.10);
            let w_out = (r_out * dphi * 0.96).max(0.12);
            let mut m = arch_wedge(w_in, w_out, dr * 0.9, rib_depth * 0.95);

            let py = spring_y + phic.sin() * r_mid;
            let pz = phic.cos() * r_mid;

            // arch_wedge 约定（已在 chapel.rs 验证）：
            //   局部 +Y = 外径向（w_top 宽的一面）。
            // (1) Ry(+90°)：切向 X → Z，深 Z → X（拱平面从 XY 转到 YZ）
            // (2) Rx(π/2−phic)：局部 +Y → 世界外径向 (0, sin φ, cos φ)
            let q = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2 - phic)
                * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
            m = m.rotated_by(q);
            m = m.translated_by(Vec3::new(x, py, pz));
            parts.push((
                m,
                if i == voussoirs / 2 { cols.dark } else { pick(cols, rib as i32, i as i32 + 400) },
            ));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 6 — 人字屋顶（两坡，脊沿 X 或沿 Z）
// ═══════════════════════════════════════════════════════════════════════════

pub fn gable_roof(
    x_range: [f32; 2],
    z_range: [f32; 2],
    base_y: f32,
    pitch: f32,
    ridge: char,          // 'x' = 脊沿 X（两坡朝南北），'z' = 脊沿 Z（两坡朝东西）
    dark:  &[f32; 4],
    light: &[f32; 4],
    parts: &mut Parts,
) {
    let thickness = 0.3;
    let cx = (x_range[0] + x_range[1]) * 0.5;
    let cz = (z_range[0] + z_range[1]) * 0.5;

    if ridge == 'x' {
        // 屋脊沿 X：Z∈[z0,z1] 的中点是脊，半跨 = half_z
        let half = (z_range[1] - z_range[0]) * 0.5;
        let rise = (z_range[1] - z_range[0]) * pitch;
        let slope_len = (half * half + rise * rise).sqrt();
        let angle = (rise / half).atan();
        let len_x = x_range[1] - x_range[0];

        // 南坡：局部 cuboid 中心 = (0, 0, 0)。长=len_x(X), 厚=thick(Y), 坡长=slope(Z)
        // 旋转 Rx(-angle)：局部 +Z 端（Z=+slope/2）仰起，转到脊线高。
        // 旋转后：板中心 Y = base_y + rise*0.5,
        //         板中心 Z = cz − half*0.5（坡 Z 向中点 = 从 cz-half 到 cz 的中点 = cz-half/2）
        let south = cuboid(len_x * 0.98, thickness, slope_len * 1.02)
            .rotated_by(Quat::from_rotation_x(-angle))
            .translated_by(Vec3::new(cx, base_y + rise * 0.5, cz - half * 0.5));
        parts.push((south, *dark));

        // 北坡：Rx(+angle)，局部 -Z 端仰起
        let north = cuboid(len_x * 0.98, thickness, slope_len * 1.02)
            .rotated_by(Quat::from_rotation_x(angle))
            .translated_by(Vec3::new(cx, base_y + rise * 0.5, cz + half * 0.5));
        parts.push((north, *light));
    } else {
        // 屋脊沿 Z（横厅臂用）：半跨 = half_x，rise = span_x * pitch
        let half = (x_range[1] - x_range[0]) * 0.5;
        let rise = (x_range[1] - x_range[0]) * pitch;
        let slope_len = (half * half + rise * rise).sqrt();
        let angle = (rise / half).atan();
        let len_z = z_range[1] - z_range[0];

        // 西坡：沿 Z 宽板，长=slope_len(X), 厚=thick(Y), 宽=len_z(Z)
        // Rz(+angle)：局部 +X 端翘起 → 转到脊线
        // 中心 X = (x_range[0] + cx)/2 = x_range[0] + half/2
        let west = cuboid(slope_len * 1.02, thickness, len_z * 0.98)
            .rotated_by(Quat::from_rotation_z(angle))
            .translated_by(Vec3::new(x_range[0] + half * 0.5, base_y + rise * 0.5, cz));
        parts.push((west, *dark));

        // 东坡：Rz(-angle)，局部 -X 端翘起
        let east = cuboid(slope_len * 1.02, thickness, len_z * 0.98)
            .rotated_by(Quat::from_rotation_z(-angle))
            .translated_by(Vec3::new(x_range[1] - half * 0.5, base_y + rise * 0.5, cz));
        parts.push((east, *light));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 7 / 8 — 后殿半圆锥顶 / 塔八棱锥台顶
// ═══════════════════════════════════════════════════════════════════════════

pub fn half_cone_roof(
    centre: [f32; 2],
    radius: f32,
    base_y: f32,
    height: f32,
    col: &[f32; 4],
    parts: &mut Parts,
) {
    // half_cone 自身坐标：平边直径沿 Z，弧朝 +X，顶点 (0, h, 0)，底在 y=0
    // 平移：底 y = base_y → 中心 y = base_y + height/2；XZ = centre
    let m = half_cone(radius * 1.02, height, 14)
        .translated_by(Vec3::new(centre[0], base_y + height * 0.5, centre[1]));
    parts.push((m, *col));
}

pub fn pyramid_roof(
    centre: [f32; 2],
    size: f32,            // 塔平面边长（正方形内切圆径 = size，外接 = size·√2/2）
    base_y: f32,
    height: f32,
    col: &[f32; 4],
    parts: &mut Parts,
) {
    let base_r = size * 0.5 * std::f32::consts::SQRT_2; // 外接圆半径，八面贴合塔身
    let m = frustum(0.05, base_r, height, 8)
        .translated_by(Vec3::new(centre[0], base_y + height * 0.5, centre[1]));
    parts.push((m, *col));
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 9 — 拱券排（结构拱 / 装饰盲拱，XY 或 YZ 平面统一）
// ═══════════════════════════════════════════════════════════════════════════
/// plane = 'x'：拱跨沿 X（拱在 XY 平面，绕 Z 轴弯），fixed = 墙面 Z 坐标。
/// plane = 'z'：拱跨沿 Z（拱在 YZ 平面，绕 X 轴弯），fixed = 墙面 X 坐标。
///
/// arch_wedge 参数约定（与 chapel.rs 一致，已视觉验证）：
///   w_bottom = 内弧长（窄，朝拱心），w_top = 外弧长（宽），height = 径向厚，depth = 墙厚
///   局部 +Y = 外径向方向。
///
/// φ ∈ [0, π]：0 = 右拱脚（沿跨轴 +R 处），π/2 = 拱顶（拱心石），π = 左拱脚（沿跨轴 -R 处）。
///   跨轴 = X（plane x）或 Z（plane z）。

pub fn arch_row(
    plane: char,
    fixed: f32,
    centres: &[f32],
    spring_y: f32,
    radius: f32,
    depth: f32,
    voussoirs: usize,
    cols: &StoneCols,
    parts: &mut Parts,
) {
    let r_in  = radius - depth * 0.5;
    let r_out = radius + depth * 0.5;
    let r_mid = (r_in + r_out) * 0.5;
    let dr = r_out - r_in;
    let dphi = std::f32::consts::PI / voussoirs as f32;

    for &c in centres {
        for i in 0..voussoirs {
            let phi0 = std::f32::consts::PI - (i as f32)       * dphi;
            let phi1 = std::f32::consts::PI - ((i as f32) + 1.0) * dphi;
            let phic = (phi0 + phi1) * 0.5;

            let w_in  = (r_in  * dphi * 0.96).max(0.10);
            let w_out = (r_out * dphi * 0.96).max(0.12);
            let mut m = arch_wedge(w_in, w_out, dr * 0.9, depth * 0.9);

            if plane == 'x' {
                // XY 平面：拱跨沿 X，fixed = Z
                // 外径向 = 世界 (cos φ, sin φ, 0)
                // 位置：跨轴 X = c + cos φ · r_mid，Y = spring_y + sin φ · r_mid
                let px = c + phic.cos() * r_mid;
                let py = spring_y + phic.sin() * r_mid;
                // 局部 +Y → 外径向 (cos φ, sin φ, 0)
                // Rz(φ − π/2)：局部 +Y 转到 (sin(φ−π/2+π/2)? 直接推：
                //   Rz(α)·+Y = (-sin α, cos α, 0)，令它 = (cos φ, sin φ, 0)
                //   → -sin α = cos φ, cos α = sin φ → α = π/2 − φ? 不对:
                //   α = π/2 - φ: sin α = cos φ, cos α = sin φ.
                //   但我们要 -sin α = cos φ → sin α = -cos φ  → α = −(π/2−φ) = φ − π/2 ✓
                m = m.rotated_by(Quat::from_rotation_z(phic - std::f32::consts::FRAC_PI_2));
                m = m.translated_by(Vec3::new(px, py, fixed));
            } else {
                // YZ 平面：拱跨沿 Z，fixed = X
                // 外径向 = 世界 (0, sin φ, cos φ)
                let py = spring_y + phic.sin() * r_mid;
                let pz = c + phic.cos() * r_mid;
                // 局部 +Y → (0, sin φ, cos φ)
                // Rx(π/2 − φ)：局部 +Y →  (0, cos(π/2−φ), sin(π/2−φ)) = (0, sin φ, cos φ) ✓
                m = m.rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2 - phic));
                m = m.translated_by(Vec3::new(fixed, py, pz));
            }
            parts.push((
                m,
                if i == voussoirs / 2 { cols.dark } else { pick(cols, i as i32, (c * 10.0) as i32) },
            ));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 原语 10 — 玫瑰窗砖环（嵌在西立面圆洞中）
// ═══════════════════════════════════════════════════════════════════════════

pub fn rose_window(
    centre_xz: [f32; 2],  // 墙面 (X, Z) — 西立面 X 固定 = centre_xz[0]
    y: f32,
    r_mid: f32,
    ring_t: f32,
    segments: usize,
    cols: &StoneCols,
    parts: &mut Parts,
) {
    let r_in  = r_mid - ring_t * 0.5;
    let r_out = r_mid + ring_t * 0.5;
    let dth   = std::f32::consts::TAU / segments as f32;
    for i in 0..segments {
        let thc = (i as f32 + 0.5) * dth;
        let w_in  = dth * r_in;
        let w_out = dth * r_out;
        let mut m = radial_wedge(w_out, w_in, ring_t * 0.9, ring_t * 0.9);
        // radial_wedge 局部 +Z = 外径向，局部 X = 切向（内/外弧沿 X 方向）。
        // 玫瑰窗位于 YZ 平面（X = centre_xz[0] 固定）：
        //   期望：外径向 = YZ 平面内从圆心向外 = (0, sin thc, cos thc) 的反？
        //   环的每块 wedge 从环心沿 thc 角向外：在局部坐标系中把局部 +Z 深度轴旋转到 YZ 平面外径向即可。
        // 操作：先 Ry(-90°) 把局部 +Z(深) 转到 -X（墙厚方向），局部 X(切向) → +Z（YZ 平面）；
        //       再 Rx(thc) 在 YZ 平面内旋转 thc，让局部 +Z(现在是 +Z 方向分量) 沿 thc。
        m = m.rotated_by(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2));
        m = m.rotated_by(Quat::from_rotation_x(thc));
        let r_cent = (r_in + r_out) * 0.5;
        let cy = y + thc.sin() * r_cent;
        let cz = centre_xz[1] + thc.cos() * r_cent;
        m = m.translated_by(Vec3::new(centre_xz[0], cy, cz));
        parts.push((m, if i % 2 == 0 { cols.mid } else { cols.light }));
    }
}
