# 程序化建筑编码教程 —— 从图纸到自己造一座建筑

> 依据 [BASILICA_PLAN.md](BASILICA_PLAN.md)（设计图纸）、[basilica.rs](src/basilica.rs)（完整示例）、[geoms.rs](src/geoms.rs)（积木库）提炼的方法论。
> 目标：读完本教程，你能独立从零造出**任何**想要的新建筑。

---

## 〇、核心理念

一座程序化建筑 = **一套坐标约定 + 一张常量表 + 一盒积木 + 分步组装 + 几何审计**。

```
画图纸 ──► 定常量 ──► 搭骨架(墙体) ──► 弯曲面(拱/圆) ──► 盖屋顶 ──► 审计验证
 ①         ②           ③               ④               ⑤         ⑥
```

三条铁律（本教程反复强调）：

1. **图纸先行**：先在 markdown 里把每个区域写成 `[坐标下界, 坐标上界]`，代码只做翻译。绝不边写代码边拍脑袋定坐标。
2. **常量只写一次**：所有尺寸集中成 `pub const`，派生尺寸必须写成公式（如 `APSE_SIDE_R = 廊宽 / 2`），不写魔法数。
3. **审计兜底**：肉眼会骗人（透视、遮挡），每个建筑配一个 audit 二进制做横截面 + 闭合断言。

---

## 一、第一步：画图纸（不写一行代码）

参照 [BASILICA_PLAN.md](BASILICA_PLAN.md) 的结构，图纸要包含 4 块内容：

### 1. 坐标系约定（写在最前面）

```
X 轴 = 长度（西 → 东，门一般开在 X 小的一端）
Z 轴 = 宽度（南 → 北）
Y 轴 = 高度，地面 y = 0
```

### 2. XZ 平面图（俯视，y=0）

用 ASCII 画平面图，**每个区域标注精确的坐标范围**。例如：

```
 Z / X   -14  ...  -10        0        +12  +16
  +10  ┌────┐
       │NW塔 │ -14..-8
  +4   │    │┌────────────────────┐┌──────┐
       └────┘│  中殿  -4..+4      ││主后殿│
   0        │  X: -10..+12       ││ R=4  │
  -4   ┌────┐│                    │└──────┘
       │SW塔 │└────────────────────┘
 -10   └────┘
```

### 3. 区域明细表（图纸的核心）

| 区域 | X 范围 | Z 范围 | 墙高 | 备注 |
|---|---|---|---|---|
| 中殿 | −10 ~ +12 | −4 ~ +4 | 9m | 8m 宽，上接筒形拱 |
| 南廊 | −10 ~ +12 | −7 ~ −4 | 6m | 3m 宽 |

**关键规则：相邻区域的共享边界必须写同一个数字。**
比如南廊 Z 上界 = 中殿 Z 下界 = −4.0，这样连拱廊柱子刚好立在边界上，不会出现缝隙或重叠。

### 4. 竖向剖面图（沿中轴切开）

标出每一层的高度线：地面 0 → 侧廊顶 6 → 连拱廊顶 6 → 起拱线 9 → 拱顶 13 → 屋脊 17.1。

**高度层级表**（从高到低排）：

| 层 | 底高 | 顶高 |
|---|---|---|
| 塔顶锥 | 15 | 18.5 |
| 中殿屋脊 | 13.5 | 17.1 |
| 拱顶 | 9（起拱） | 13（拱冠） |
| 侧廊屋脊 | 6 | 8.7 |

### 何时停下来检查

图纸画完，自查三件事：
- 每个区域是否都有唯一的（X 范围, Z 范围, 高度）三元组？
- 拼起来后有没有**重叠**（同一块空间被两个区域占据）或**缝隙**（两区域之间漏空）？
- 曲线构件（拱、后殿）的半径和直线尺寸是否对得上？如：后殿直径 = 廊宽。

---

## 二、第二步：常量翻译（图纸 → 代码）

把图纸表格逐行翻译成 Rust 常量，放在文件开头。对照 [basilica.rs](src/basilica.rs#L16-L91)：

```rust
// ── 区域边界：直接抄图纸 ──
pub const NAVE_X_WEST: f32 = -10.0;
pub const NAVE_X_EAST: f32 =  12.0;   // 22 m 长
pub const NAVE_Z_LO:   f32 = -4.0;    // 8 m 宽
pub const NAVE_Z_HI:   f32 =  4.0;

// ── 派生常量：必须写成公式，并注释推导 ──
pub const APSE_MAIN_R: f32 = 4.0;   // = NAVE 宽 8 m ÷ 2，后殿直径贴齐中殿
pub const APSE_SOUTH_CZ: f32 = -5.5; // = (AISLE_Z_S_LO + AISLE_Z_S_HI) / 2 廊中点

// ── 材料规格 ──
pub const WALL_T_MAIN:  f32 = 1.0;  // 主墙厚
pub const BLOCK_H_NOM:  f32 = 0.5;  // 砖层高（一层 50cm）
pub const BLOCK_W_NOM:  f32 = 1.0;  // 砖名义宽
```

**命名规范**（沿用 basilica.rs）：
- 边界：`<区域>_X_WEST / _X_EAST / _Z_LO / _Z_HI`
- 高度：`<区域>_WALL_H`
- 曲线：`半径 _R`、`圆心 _CZ`、`拱起拱 VAULT_SPRINGING_Y`

**派生常量的注释里写清"为什么"**——三个月后你自己会感谢这行注释。

---

## 三、第三步：认识积木箱（geoms.rs）

[geoms.rs](src/geoms.rs) 是共享积木库，分三层：

### 1. 基础原语（Bevy 自带，直接用）

| 函数 | 用途 |
|---|---|
| `cuboid(sx, sy, sz)` | 方砖、楼板、屋檐 |
| `frustum(rt, rb, h, res)` | 圆台/圆柱/锥（塔顶、柱子） |
| `cone(r, h, res)` | 后殿锥顶 |
| `ball(r)` | 装饰球 |

### 2. 异形砖（手工构网，见 [geoms.rs L92-211](src/geoms.rs#L92-L211)）

| 函数 | 形状 | 典型用途 |
|---|---|---|
| `arch_wedge(w_bottom, w_top, height, depth)` | 沿 Y 收窄的楔形 | 拱券石、筒形拱肋 |
| `radial_wedge(w_outer, w_inner, height, depth)` | 沿径向（Z）收窄的梯形砖 | 后殿弧墙、玫瑰窗环 |
| `tetra_brick(a, b, c)` | 直角四面体 | 转角填缝、破损 |
| `octagon_prism(r, h)` | 八角柱 | 连拱廊柱 |

> ⚠️ **大坑预警**：手工 Mesh 必须补齐 `NORMAL` 和 `UV_0` 属性（数量与顶点一致），否则 `merge()` 时属性不匹配会**静默丢顶点**——表现为"构件莫名消失"。geoms.rs 的 `build_mesh()` 已处理，照抄即可。

### 3. 砌墙系统（核心中的核心）

```rust
rubble_wall(
    along,               // 'x' = 墙沿 Z 走向(固定X) | 'z' = 墙沿 X 走向(固定Z)
    base_x, base_z,      // 墙起点（对应固定轴的坐标 + 走向轴的起点）
    len,                 // 墙长
    wall_h,              // 墙高
    drop,                // 0.0 = 完整建筑; >0 = 破损比例（废墟用）
    cols,                // 石头三色组
    voids,               // &[WallVoid] 墙上开洞（门/窗）
    parts,               // 输出累积器
    skip_along_ranges,   // 跳过区间（防止与相邻墙体重叠绘制）
    block_h, wall_t, block_w_nom,  // 砖层高/墙厚/砖宽
);
```

它自动完成：分层（running bond 错缝）、砖块随机尺寸（±25%）、轻微旋转抖动（±1.8°）、三色随机上色。完整建筑包一层 `intact_wall`（drop=0）。

**墙上开洞 = `WallVoid` 列表**（along 是沿墙距离，y 是世界高度）：

```rust
// 一扇带半圆拱的门 = 两个矩形洞拼近似
voids.push(WallVoid { along_lo: c - 0.8, along_hi: c + 0.8, y_lo: 0.0,  y_hi: 3.8 });  // 门洞
voids.push(WallVoid { along_lo: c - 0.8, along_hi: c + 0.8, y_lo: 3.8,  y_hi: 5.6 });  // 上方拱段
```

### 4. Mesh 管线（性能关键）

```rust
// 所有构件塞进同一个 Vec
let mut parts: Vec<(Mesh, [f32; 4])> = Vec::new();
parts.push((砖块Mesh, 颜色));

// 最后一次性合并 → 平面着色 → 单 Mesh 返回
mesh_from_parts(parts)
```

一个建筑 = 一个 Mesh = 一次 draw call。顶点色（`lin(0x9a968a)`）代替材质，全部共用一个白色 StandardMaterial，数千块砖自动合批。

---

## 四、第四步：分步组装（骨架代码结构）

采用 basilica.rs 的编排模式——**一个总入口 + 每个构件一个 build 函数**：

```rust
pub fn build_my_building() -> Vec<Mesh> {
    let cols = stone_cols();
    let mut parts: Vec<(Mesh, [f32; 4])> = Vec::new();

    build_walls(&cols, &mut parts);      // Step 1: 墙体骨架
    build_openings(&cols, &mut parts);   // Step 2: 拱门/窗装饰
    build_columns(&cols, &mut parts);    // Step 3: 柱子
    build_vault(&cols, &mut parts);      // Step 4: 拱顶
    build_roofs(&mut parts);             // Step 5: 屋顶
    add_decorations(&cols, &mut parts);  // Step 6: 装饰（盲拱/苔藓）

    vec![mesh_from_parts(parts)]
}
```

每个 `build_xxx` 只做一件事，签名统一 `(cols, &mut parts)`。然后在 main.rs 里替换入口：

```rust
// main.rs
for mesh in basilica::build_basilica() { /* spawn */ }
// 改成
for mesh in my_building::build_my_building() { /* spawn */ }
```

**推荐搭建顺序：先墙后顶、先直后曲**。墙体立起来就能跑一次看效果，再逐步加曲面和屋顶。

---

## 五、第五步：弯曲构件的通用套路（最难也最有用）

所有曲线构件（拱、环、后殿、拱顶）都是同一个三步法：

### 通用三步法

```rust
// ① 把圆分成 N 段，算出每段的中心角 thc
let n = 7;                                // 奇数！保证正中间是拱心石
let dth = PI / n as f32;                  // 半圆用 PI，整圆用 TAU
for i in 0..n {
    let th0 = start + i as f32 * dth;
    let th1 = start + (i + 1) as f32 * dth;
    let thc = (th0 + th1) * 0.5;          // 段中心角

    // ② 楔形砖尺寸按弧长算：外弧长 > 内弧长
    let w_out = r_out * dth;
    let w_in  = r_in  * dth;

    // ③ 砖心放到 (圆心 + r_mid × 方向向量)，再旋转让径向朝外
    let px = cx + thc.cos() * r_mid;
    let pz = cz + thc.sin() * r_mid;
    let rot = FRAC_PI_2 - thc;            // 径向朝外的旋转角
    let mut m = radial_wedge(w_out, w_in, block_h, wall_t);
    m = m.rotated_by(Quat::from_rotation_y(rot));
    m = m.translated_by(Vec3::new(px, y, pz));
    parts.push((m, pick(cols, i as i32, y as i32)));
}
```

### 两个现成范例

**后殿半圆墙**（[basilica.rs L565-597](src/basilica.rs#L565-L597)）：XZ 平面上的半圆，θ ∈ [−π/2, +π/2]，`Ry(π/2 − thc)` 旋转让径向朝外。每层再沿 Y 重复砌一圈。

**筒形拱肋**（[basilica.rs L603-673](src/basilica.rs#L603-L673)）：YZ 平面上的半圆，比后殿多一次坐标变换：

```rust
// arch_wedge 原生在 XY 平面 → 先 Ry(+90°) 转到 YZ 平面，再 Rx(π/2 − phic) 定位极角
let q_total = Quat::from_rotation_x(FRAC_PI_2 - phic)
            * Quat::from_rotation_y(FRAC_PI_2);   // 先 orient 后 tilt
m = m.rotated_by(q_total);
```

### 易错点（全是踩过的坑）

| 坑 | 正确做法 |
|---|---|
| 拱肋角度范围写 [−π/2, +π/2] 导致下半埋地 | **phic ∈ [π → 0]**，sin ≥ 0，拱永远在起拱线上方 |
| 楔形砖径向朝内（外弧在内侧） | 旋转角用 **α = π/2 − thc**，砖的外弧面朝外 |
| 拱券用偶数块砖 | 用**奇数**（7/9 块），正中是拱心石 |
| 旋转顺序随意 | Bevy 四元数 `q_total * v = q_tilt * (q_orient * v)`，**先定向再倾斜** |

---

## 六、第六步：屋顶

### 人字顶（gable）= 两块旋转的 cuboid

核心公式（[basilica.rs L742-778](src/basilica.rs#L742-L778)）：

```rust
let rise = span_z * pitch;                       // 屋脊抬高量
let angle = (rise / half_z).atan();              // 坡角
let slope_len = (half_z² + rise²).sqrt();        // 斜坡长度

// 南坡：Rx(−angle)  北坡：Rx(+angle) —— 方向相反才能在 Z=0 会合成脊
let mut south = cuboid(len_x, 0.3, slope_len);
south = south.rotated_by(Quat::from_rotation_x(-angle));
south = south.translated_by(Vec3::new(cx, base_y + rise * 0.5, cz - half_z * 0.5));
```

> ⚠️ **南坡必须 Rx(−angle)、北坡 Rx(+angle)**。符号写反 → 两坡向上翻开像一本打开的书（真实翻过的坑）。

### 锥顶（塔顶/后殿顶）

```rust
// 后殿：直接用 cone，16 面近似
let cone_h = base_r * 0.85 + 0.5;
cone(base_r * 1.02, cone_h, 16)
    .translated_by(Vec3::new(cx, base_y + cone_h * 0.5, cz))

// 塔顶：方塔对角线换算 circumradius，8 面棱锥
let base_r = TOWER_SIZE * 0.5 * SQRT_2;
frustum(0.05, base_r, 3.5, 8)
```

**屋顶必须"坐在"结构上**：base_y 取墙顶/雉堞顶/拱冠 +0.01~0.5m，悬浮屋顶一眼假。

---

## 七、第七步：审计验证（肉眼不可信）

参照 [basilica_audit.rs](src/bin/basilica_audit.rs) 的套路，给每个建筑配一个 `src/bin/<name>_audit.rs`：

### 三种审计手段

**1. 水平横截面**（最直观）——在关键高度切一刀，打印占据格：

```rust
// 在 y = 5.0 切一刀，步长 0.5m，扫描 XZ 平面
for z in (-10..=10).step_by(1) {
    for x in (-16..=16).step_by(1) {
        // 查询该点是否被任一墙体实体覆盖 → 打印 █ 或 ·
    }
}
```

对照图纸逐层检查：y=1 应看到完整闭环；y=7 只有中殿+横厅；y=10 只有塔。

**2. 闭合断言**——沿建筑外圈走一圈，每个采样点都必须"内外有别"：

```rust
// 从 (X_MIN−2, 0) 沿 +X 走到 (X_MAX+2, 0)，穿过的实体表面次数必须是偶数
// 奇数 = 有墙没闭合（漏了个面）
assert_eq!(crossings % 2, 0, "南墙在 Z=0 处不闭合");
```

**3. 参数核对**——把图纸"关键尺寸速查"逐条翻译成断言：

```rust
assert!((APSE_MAIN_R * 2.0 - (NAVE_Z_HI - NAVE_Z_LO)).abs() < 1e-6);  // 主后殿直径=中殿宽
assert!((VAULT_SPRINGING_Y + VAULT_RADIUS - 13.0).abs() < 1e-6);      // 拱冠=13
```

> ⚠️ 审计脚本的常量必须从 basilica.rs `use` 进来，**不要手抄**。手抄旧常量 = 错误断言（翻过的坑）。

---

## 八、完整工作流 CheckList

```
□ 1. 画图纸：平面图 + 区域表 + 剖面图 + 高度层级表
□ 2. 自查：无重叠、无缝隙、曲线半径与直线尺寸自洽
□ 3. 翻译常量：边界直抄，派生量写公式 + 注释
□ 4. 搭墙体：intact_wall × N，用 WallVoid 开门窗，skip 防重叠
□ 5. 跑一次！先看骨架对不对，再继续
□ 6. 弯曲面：三步法（分段→楔形砖→旋转平移）
□ 7. 盖屋顶：gable 两坡符号相反，锥顶坐实
□ 8. 装饰：盲拱、雉堞、玫瑰窗（全部复用三步法）
□ 9. 写 audit bin：横截面 + 闭合断言 + 尺寸断言
□ 10. cargo run 肉眼终审
```

---

## 九、实战：从零造一座小钟塔（全流程演示）

用上面的方法造一座 4×4m、高 10m 的独立钟塔。**先画图**：

```
平面（XZ, y=0）:  X ∈ [-2, +2], Z ∈ [-2, +2]   4×4m
高度层级:         墙体 0~8m → 雉堞 8~9m → 锥顶 9~12m
门:              南墙 Z=-2, 拱门宽1.2m 高2.5m
窗:              每面一扇拱窗, y=5~7
```

**翻译成代码**（新建 `src/campanile.rs`）：

```rust
use bevy::prelude::*;
use crate::geoms::*;

// ── 常量（图纸直抄）──
pub const T_X_LO: f32 = -2.0;
pub const T_X_HI: f32 =  2.0;
pub const T_Z_LO: f32 = -2.0;
pub const T_Z_HI: f32 =  2.0;
pub const T_WALL_H: f32 = 8.0;
pub const DOOR_W: f32 = 1.2;    // = 拱直径 → 拱心石刚好在 y=2.5+0.6
pub const DOOR_H: f32 = 2.5;

pub fn build_campanile() -> Vec<Mesh> {
    let cols = stone_cols();
    let mut parts: Vec<(Mesh, [f32; 4])> = Vec::new();
    let len = T_X_HI - T_X_LO;

    // ── Step 1: 四面墙。南墙开门（沿墙距离 = 门中心 Z），其余开窗 ──
    let door = [
        WallVoid { along_lo: len/2. - DOOR_W/2. - 0.02, along_hi: len/2. + DOOR_W/2. + 0.02,
                   y_lo: 0.0, y_hi: DOOR_H },
        WallVoid { along_lo: len/2. - DOOR_W/2. - 0.05, along_hi: len/2. + DOOR_W/2. + 0.05,
                   y_lo: DOOR_H, y_hi: DOOR_H + DOOR_W/2. },
    ];
    let window = [ WallVoid { along_lo: len/2. - 0.4, along_hi: len/2. + 0.4, y_lo: 5.0, y_hi: 6.8 } ];

    intact_wall('z', T_X_LO, T_Z_LO, len, T_WALL_H, cols, &door,   parts, &[], 0.5, 1.0, 1.0); // 南
    intact_wall('z', T_X_LO, T_Z_HI, len, T_WALL_H, cols, &window, parts, &[], 0.5, 1.0, 1.0); // 北
    intact_wall('x', T_X_LO, T_Z_LO, len, T_WALL_H, cols, &window, parts, &[], 0.5, 1.0, 1.0); // 西
    intact_wall('x', T_X_HI, T_Z_LO, len, T_WALL_H, cols, &window, parts, &[], 0.5, 1.0, 1.0); // 东

    // ── Step 2: 门拱装饰（三步法，7 块拱心石）──
    let arch_r = DOOR_W / 2.0;
    let r_in = arch_r - 0.5;  let r_out = arch_r + 0.5;
    let n = 7;  let dphi = std::f32::consts::PI / n as f32;
    for i in 0..n {
        let phic = -FRAC_PI_2 + (i as f32 + 0.5) * dphi;
        let r_mid = (r_in + r_out) * 0.5;
        let w_bot = r_mid * dphi;
        let mut m = arch_wedge(w_bot, w_bot * (r_in / r_out).max(0.55), 0.45, 0.9);
        m = m.rotated_by(Quat::from_rotation_x(FRAC_PI_2 - phic));
        m = m.translated_by(Vec3::new(T_X_LO, DOOR_H + phic.sin() * r_mid, phic.cos() * r_mid));
        parts.push((m, if i == n / 2 { cols.dark } else { pick(cols, i as i32, 7) }));
    }

    // ── Step 3: 雉堞（沿每面墙交替放实体块）──
    for (along, fx, fz) in [('z', T_X_LO, T_Z_LO), ('z', T_X_LO, T_Z_HI),
                             ('x', T_X_LO, T_Z_LO), ('x', T_X_HI, T_Z_LO)] {
        let mut a = -1.5;
        while a < 2.0 {
            let m = match along {
                'z' => cuboid(0.95, 0.95, 0.95).translated_by(Vec3::new(T_X_LO + 2.0 + a.clamp(-99.,99.) * 0.0, 8.5, fz + a + 0.0)),
                _   => cuboid(0.95, 0.95, 0.95).translated_by(Vec3::new(fx + a, 8.5, T_Z_LO + 2.0)),
            };
            let _ = along; // 简化示例：按面循环放置 merlon
            parts.push((m, cols.dark));
            a += 2.0;
        }
    }

    // ── Step 4: 四棱锥顶（坐在雉堞上）──
    let base_r = 2.0 * std::f32::consts::SQRT_2;  // 方塔对角半径
    let cone = frustum(0.05, base_r, 3.0, 4)
        .translated_by(Vec3::new(0.0, 9.0 + 1.5, 0.0));
    parts.push((cone, lin(ROOF)));

    vec![mesh_from_parts(parts)]
}
```

（雉堞部分简化示意，实际写四个循环更清晰；完整参考 [build_single_tower](src/basilica.rs#L274-L319)。）

然后在 main.rs 挂上入口、`cargo run`、写 `campanile_audit.rs` 验证四墙闭合 + 门洞位置。**这就是完整流程**。

---

## 十、进阶方向

- **废墟化**：把 `intact_wall` 换回 `rubble_wall(drop = 0.3)`，顶部自动参差剥落；散砖按权重撒在受损墙脚（参考 chapel.rs）。
- **建筑变体**：常量不变，函数加参数。比如 `build_apse(cx, cz, r, ...)` 传不同半径/段数即得主/侧后殿。
- **数据驱动**：把常量表抽成 JSON 的 `BuildingBlueprint`（已在规划中），代码只保留构件生成器，实现"一份 JSON = 一座建筑"。
- **地形融合**：墙体 base_y 接 `terrain_height(x, z)`，而不是硬编码 0。

---

## 附：常见坑速查表

| 症状 | 原因 | 修复 |
|---|---|---|
| 构件消失 | 手工 Mesh 缺 NORMAL/UV_0，merge 丢顶点 | 用 `build_mesh()` 补占位属性 |
| 拱下半截埋地 | phic 范围含负 sin | phic ∈ [π, 0] |
| 屋顶像翻开的书 | 两坡旋转同号 | 南 Rx(−a) 北 Rx(+a) |
| 后殿砖缝外宽内窄反了 | 径向朝内 | α = π/2 − thc |
| 墙面有洞 | WallVoid 用世界坐标但 along 用墙局部坐标 | along = 目标Z − base_z |
| 塔身砖隔层凸出 | 相邻墙共享平面重复绘制 | `skip_along_ranges` 跳过共享段 |
| 审计报错但看着没问题 | audit 脚本手抄了旧常量 | `use crate::basilica::*` 同步 |
| 启动极慢 | 256² 纹理多层噪声 | 降到 64² |
