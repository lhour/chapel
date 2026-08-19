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

/// Build the full chapel model and return the finished meshes (already merged+flat+coloured).
pub fn build_chapel() -> Vec<Mesh> {
    let cols = stone_cols();

    // ── Door & arch geometry (shared between west-wall void and arch builder) ─
    // Door: 1.6 wide × 3.0 tall, centred at Z = -0.5 on the west wall
    // West wall runs along Z, base_z = -NAVE_W/2 = -5.0, len = 10.0 → along-axis = Z + 5.0.
    // Door centre Z = -0.5 → along centre = 4.5 m.  along ∈ [3.7, 5.3].
    const DOOR_H: f32 = 3.0;
    const DOOR_W: f32 = 1.6;
    const DOOR_ALONG_C: f32 = (NAVE_W / 2.0) + (-0.5);
    let door_void = WallVoid {
        along_lo: DOOR_ALONG_C - DOOR_W / 2.0,
        along_hi: DOOR_ALONG_C + DOOR_W / 2.0,
        y_lo: 0.0,
        y_hi: DOOR_H,
    };
    // Arch semicircle above door: radius = DOOR_W / 2 = 0.8 m, centre at (y=3.0, along=4.5)
    let arch_void = WallVoid {
        along_lo: DOOR_ALONG_C - DOOR_W / 2.0 - 0.02,
        along_hi: DOOR_ALONG_C + DOOR_W / 2.0 + 0.02,
        y_lo: DOOR_H - 0.02,
        y_hi: DOOR_H + DOOR_W / 2.0 + 0.02,
    };

    // ── Nave walls ──────────────────────────────────────────────────────────
    // Coordinate system (matches layout constants):
    //   X axis = nave length 16 m,  east = +8,  west = -8
    //   Z axis = nave width  10 m,  north = +5, south = -5
    // Walls are axis-aligned; corners: SW(-8,-5) SE(8,-5) NW(-8,5) NE(8,5)
    let mut nave: Vec<(Mesh, [f32; 4])> = Vec::new();

    // West-wall geometry: skip the along-range [0, 4] (Z = -5 .. -1) because
    // that stretch is drawn by the tower's west face (no double-draw → no flicker).
    let west_wall_skip = [(0.0_f32, 4.0)];

    // East-wall geometry: leave a full-height opening for the apse at Z ∈ [-4,+4].
    // East wall runs along Z, base_z = -5, len = 10.  Z ∈ [-4,+4] → along ∈ [1, 9].
    let apse_opening = WallVoid { along_lo: 1.0, along_hi: 9.0, y_lo: 0.0, y_hi: WALL_H + 0.1 };

    // ── Long walls (north & south): run along X, fixed Z = ±W/2 ──
    rubble_wall('z', -NAVE_L / 2.0, -NAVE_W / 2.0, NAVE_L, WALL_H, 0.35, &cols, &[], &mut nave, &[], BLOCK_H, WALL_T, BLOCK_W); // South
    rubble_wall('z', -NAVE_L / 2.0,  NAVE_W / 2.0, NAVE_L, WALL_H, 0.20, &cols, &[], &mut nave, &[], BLOCK_H, WALL_T, BLOCK_W); // North
    // ── Short walls (west & east): run along Z, fixed X = ±L/2 ──
    rubble_wall('x', -NAVE_L / 2.0, -NAVE_W / 2.0, NAVE_W, WALL_H, 0.45, &cols, &[door_void, arch_void], &mut nave, &west_wall_skip, BLOCK_H, WALL_T, BLOCK_W); // West
    rubble_wall('x',  NAVE_L / 2.0, -NAVE_W / 2.0, NAVE_W, WALL_H, 0.25, &cols, &[apse_opening],        &mut nave, &[], BLOCK_H, WALL_T, BLOCK_W); // East (apse opening)

    // ── Apse (east end): half-round drum using true radial wedge bricks ────
    // Centre of apse half-circle = (X = NAVE_L/2=8, Z=0), APSE_R = 4.0 (midline).
    // Each course is 8 true-trapezoid radial_wedges: inner face narrower, outer face
    // wider — no internal gaps, unlike rotated cuboids.
    let apse_seg: usize = 8;
    let r_in  = APSE_R - WALL_T * 0.5;   // inner radius (narrow face)
    let r_out = APSE_R + WALL_T * 0.5;   // outer radius (wide face)
    let dtheta = std::f32::consts::PI / apse_seg as f32;  // angular width of 1 segment
    for seg in 0..apse_seg {
        // seg 0 = north end (th = +π/2), seg 7 = south end (th = -π/2)
        let th0 =  std::f32::consts::FRAC_PI_2 - (seg as f32)       * dtheta;
        let th1 =  std::f32::consts::FRAC_PI_2 - ((seg as f32) + 1.0) * dtheta;
        let thc = (th0 + th1) * 0.5;
        // arc lengths on inner & outer cylindrical surfaces
        let w_in_face  = dtheta * r_in;
        let w_out_face = dtheta * r_out;
        // centreline position of this wedge (mid-radius = APSE_R)
        let cx_mid = NAVE_L / 2.0 + thc.cos() * APSE_R;
        let cz_mid = thc.sin() * APSE_R;
        // Rotation: radial_wedge's LOCAL +Z depth axis must point radially OUTWARD
        // i.e. world direction (cos thc, 0, sin thc).
        // Quat::from_rotation_y(α) takes LOCAL +Z → WORLD (sin α, 0, cos α).
        // Solve sin α = cos thc AND cos α = sin thc  →  α = π/2 - thc.
        let rot_angle = std::f32::consts::FRAC_PI_2 - thc;

        for cy in 0..((WALL_H / BLOCK_H).ceil() as i32) {
            let y = cy as f32 * BLOCK_H + BLOCK_H * 0.5;
            let top_frac = cy as f32 / (WALL_H / BLOCK_H).floor().max(1.0);
            if hash2(seg as i32, cy) < 0.35 && top_frac > 0.7 { continue; }
            // real radial wedge: outer face wider, inner face narrower
            let mut m = radial_wedge(w_out_face, w_in_face, BLOCK_H * 0.9, WALL_T * 0.9);
            m = m.rotated_by(Quat::from_rotation_y(rot_angle));
            m = m.translated_by(Vec3::new(cx_mid, y, cz_mid));
            nave.push((m, pick(&cols, seg as i32, cy)));
        }
    }

    // ── Semicircular arch above the west door (arch_wedge voussoirs) ────────
    // Arch plane = YZ (west wall), centre = (x=-8, y=3.0, z=-0.5), radius = 0.8 m.
    // 7 voussoirs (odd so the centre one is the keystone).
    {
        let arch_n: usize = 7;
        let arch_r = DOOR_W / 2.0;                      // 0.8 m
        let arch_cx = -NAVE_L / 2.0;                     // X = -8 (west wall plane)
        let arch_cy = DOOR_H;                             // Y = 3.0 (top of door opening)
        let arch_cz = -0.5;                               // Z centre matches door
        let r_arch_in  = arch_r - WALL_T * 0.5;           // 0.8 - 0.4 = 0.4
        let r_arch_out = arch_r + WALL_T * 0.5;           // 0.8 + 0.4 = 1.2
        let dphi = std::f32::consts::PI / arch_n as f32;  // π/7 per voussoir

        for i in 0..arch_n {
            // phi ∈ [-π/2, +π/2] in YZ plane: phi=0 → up (+Y), +π/2 → right (+Z)
            let phi_0 = -std::f32::consts::FRAC_PI_2 + (i as f32)       * dphi;
            let phi_1 = -std::f32::consts::FRAC_PI_2 + ((i as f32) + 1.0) * dphi;
            let phi_c = (phi_0 + phi_1) * 0.5;
            let (s, c) = phi_c.sin_cos();
            // arc lengths: near-centre face (Y-) = narrow, far-centre face (Y+) = wide
            // arch_wedge(w_bottom, w_top, height, depth): bottom=Y-, top=Y+ (LOCAL)
            // LOCAL Y = radial direction: Y- = near arch centre (narrow), Y+ = outside (wide)
            let w_narrow = dphi * r_arch_in;
            let w_wide   = dphi * r_arch_out;
            // Direction basis vectors in WORLD:
            //   radial outward (+Y local)       = (0, cos, sin)
            //   tangent (+X local) = radial × wall-normal = (0,cos,sin) × (-1,0,0) = (0, -sin, cos)
            //   wall depth (+Z local)           = (-1, 0, 0) (points OUTSIDE the nave)
            let basis = Mat3::from_cols(
                Vec3::new(0.0, -s,  c),    // +X local = tangent
                Vec3::new(0.0,  c,  s),    // +Y local = radial out
                Vec3::new(-1.0, 0.0, 0.0), // +Z local = wall normal (out)
            );
            let mut m = arch_wedge(w_narrow, w_wide, WALL_T * 0.9, WALL_T * 0.9);
            m = m.rotated_by(Quat::from_mat3(&basis));
            // Wedge centre = arch centre + mid-radius * radial direction
            m = m.translated_by(Vec3::new(arch_cx, arch_cy, arch_cz) + Vec3::new(0.0, c, s) * arch_r);
            // slightly recessed OUTSIDE the wall plane so it visually sits proud
            m = m.translated_by(Vec3::new(-0.08, 0.0, 0.0));
            nave.push((m, pick(&cols, i as i32, 909)));
        }
    }

    // ── West tower ──────────────────────────────────────────────────────────
    // A squat square tower built INTO the south-west corner of the nave: the
    // tower's WEST wall lines up flush with the nave's west wall (X = -L/2),
    // so the tower partly sits inside the nave floor (X from -8 to -3) and
    // partly bulges south of the nave (Z from -6 to -1, i.e. the south 1m of
    // the nave + 1m outside).  This way the tower is structurally joined to
    // the nave — not a disconnected shed in the yard.
    let tw = TOWER_W;
    let tx = -NAVE_L / 2.0;           // tower west side = nave west wall (X = -8)
    let tz = -NAVE_W / 2.0 - 1.0;     // tower south of nave (Z from -6  to  -1, 5m wide)
    let courses = (TOWER_H / BLOCK_H).ceil() as i32;
    // All four faces of the tower box are drawn (so every corner has two
    // walls meeting and no "sticking out half-block" artefacts on the free
    // ends of the former 2-face tower).
    //   which='x' → two faces at X = tx (west) & X = tx+tw (east), running along Z.
    //   which='z' → two faces at Z = tz (south) & Z = tz+tw (north), running along X.
    let tower_face_offsets: [(char, f32, f32); 4] = [
        ('x', tx,      tz),   // west  face: X=tx,     Z runs tz..tz+tw
        ('x', tx+tw,   tz),   // east  face: X=tx+tw,  Z runs tz..tz+tw
        ('z', tz,      tx),   // south face: Z=tz,     X runs tx..tx+tw
        ('z', tz+tw,   tx),   // north face: Z=tz+tw,  X runs tx..tx+tw
    ];
    for (which, fix_ax, along_start) in tower_face_offsets {
        let along_len = tw;
        let nblocks   = (along_len / BLOCK_W).ceil() as i32;
        let mut tv: Vec<(Mesh, [f32; 4])> = Vec::new();
        for k in 0..courses {
            let y = k as f32 * BLOCK_H + BLOCK_H * 0.5;
            let top_frac = k as f32 / courses.max(1) as f32;
            // ragged crown: collapse starts above ~80%
            if top_frac > 0.8 {
                if hash2(k, which as u8 as i32 * 3 + fix_ax as i32) < 0.6 { continue; }
            }
            for b in 0..nblocks {
                let off = if k % 2 == 0 { 0.0 } else { BLOCK_W * 0.5 };
                let p = b as f32 * BLOCK_W + off;
                // Cut off the last block of odd courses so it never sticks out
                // beyond the face end (the "half-block poking out" artefact).
                if p + BLOCK_W * 0.48 > along_len + 0.05 { continue; }
                let (px, pz) = if which == 'x' {
                    (fix_ax,        along_start + p) // along Z
                } else {
                    (along_start + p, fix_ax)        // along X
                };
                let mut m = if which == 'x' {
                    cuboid(WALL_T, BLOCK_H * 0.9, BLOCK_W * 0.92)
                } else {
                    cuboid(BLOCK_W * 0.92, BLOCK_H * 0.9, WALL_T)
                };
                m = m.translated_by(Vec3::new(px, y, pz));
                // Darker stone for the very first & last block of every course →
                // visually emphasises corners WITHOUT drawing overlapping cubes.
                let is_end = b == 0 || (p + BLOCK_W * 0.48 >= along_len - 0.05);
                let col = if is_end { cols.dark } else { pick(&cols, b, k * 5 + which as u8 as i32 + fix_ax as i32) };
                tv.push((m, col));
            }
        }
        nave.extend(tv);
    }

    // ── Broken roof (a partial shed roof over the WEST half of the nave) ────
    // A broad low slab slanted EAST-TO-WEST downwards (rot around Z axis = 0.28rad),
    // supported by three rafter beams that sit on top of the north/south walls.
    // Roof covers the western 55% of the nave width so ~half the interior is exposed
    // to the sky (matching the "half-ruined" feel).
    let roof_w = NAVE_W * 0.55;    // Z extent of roof slab (5.5 m — covers west half of width)
    let roof_l = NAVE_L * 0.9;     // X extent of roof slab (14.4 m, almost full 16 m)
    let roof_start_x = -NAVE_L / 2.0 + NAVE_L * 0.05;
    // roof rests on walls: wall top course centre is at y = WALL_H - BLOCK_H/2 (top course
    // centre of a 6.0m wall built with 0.5m courses — course 11 centre = 5.75).  We put the
    // rafters so their top face is at y ≈ 6.0 — same level as the wall — and the roof slab
    // sits directly on them.
    let rafter_top_y = 6.0;
    let rafter_h = 0.5;
    // Three rafters span the FULL nave width (north wall crown → south wall crown)
    // so they actually support the roof and can't be mistaken for a door lintel.
    // Place them from the nave mid-point eastward so they don't show through
    // the ragged top of the west-wall ruin (which was the "wood plank blocks door" illusion).
    for i in 0..3 {
        let x = -2.0 + i as f32 * 4.0;  // X = -2, +2, +6 — safely inside the nave, east of west wall
        let mut beam = cuboid(0.18, rafter_h, NAVE_W * 0.92);  // Z-length ~ 9.2 m span, sits on both walls
        let beam_cy = rafter_top_y - rafter_h * 0.5;
        beam = beam.translated_by(Vec3::new(x, beam_cy, 0.0));  // centred on nave width (Z=0)
        // Slight clockwise tilt around X so top face matches the shed roof's ~16° slope
        // (rot around X = tilts beam's cross-section, keeping it horizontal in XZ)
        beam = beam.rotated_by(Quat::from_rotation_x(0.10));
        nave.push((beam, lin(WOOD)));
    }
    // roof as a tilted slab; bottom face of slab sits ~1 cm above rafter tops.
    let slab_t = 0.15;
    let roof_cy = rafter_top_y + slab_t * 0.5 + 0.01;
    let mut roof = cuboid(roof_l, slab_t, roof_w);  // X-length, Y-thick, Z-width
    // centre slab at (mid-X, roof_cy, south-of-centre to cover the WEST half)
    let centre_x = roof_start_x + roof_l * 0.5;
    let centre_z = -NAVE_W * 0.2 + roof_w * 0.5 - roof_w * 0.5; //  -NAVE_W*0.2 = -2.0, +0
    // i.e. the roof slab's Z centre is at nave -2.0 (south-of-centre, west half)
    roof = roof.translated_by(Vec3::new(centre_x, roof_cy, -NAVE_W * 0.2));
    roof = roof.rotated_by(Quat::from_rotation_z(0.28)); // east-high → west-low shed
    nave.push((roof, lin(ROOF)));

    // ── West door (dark opening in the front wall) ──────────────────────────
    // West wall runs along Z at X = -L/2 = -8.  A door is a thin dark slab that
    // shares the wall's normal (X direction).  Door dimensions: 1.6 m wide (along Z,
    // i.e. running along the wall), 3.0 m tall, 0.2 m thick (wall normal direction,
    // a bit thinner than WALL_T = 0.8 so it sits recessed).
    // Centred on the west wall in Z, just slightly SOUTH of the nave's north-south
    // midline (visually balanced, the tower is on the south corner so the door is
    // nudged a little north of dead centre).
    {
        let door_w = 1.6;     // along Z
        let door_h = 3.0;     // along Y
        let door_t = 0.2;     // along X (recessed slightly OUTSIDE the nave wall plane)
        let mut door = cuboid(door_t, door_h, door_w);
        // put it flush with the OUTER face of the west wall — west wall bricks have their
        // centre at X = -8 and their X thickness is WALL_T so the outer face of the west
        // wall is at X = -8 - WALL_T/2 = -8.4.  Door is placed just outside that at -8.5.
        let dx = -NAVE_L / 2.0 - WALL_T / 2.0 - 0.05; // X = -8.45
        let dy = door_h / 2.0;                         // bottom rests on the floor (y=0)
        let dz = -0.5;                                  // 0.5 m north of dead centre for balance
        door = door.translated_by(Vec3::new(dx, dy, dz));
        nave.push((door, lin(DOOR)));
    }

    // ── scattered rubble at the base of the broken walls ────────────────────
    // Heaviest rubble at the south wall (Z = -5, drop = 35%) and west wall
    // (X = -8, drop = 45%) — rubble stays within ~2 m of the wall base so it
    // looks like it actually fell off the wall, not drifted in from Mars.
    // Smaller amounts at the other two walls (drop 20-25%).
    for i in 0..26 {
        // pick a wall: 45% south wall, 35% west wall, 10% north, 10% east
        // (proportional roughly to drop fractions so the most broken walls have most rubble)
        let pick = hash2(i, 77);
        let (x, z) = if pick < 0.45 {
            // SOUTH WALL BASE — Z = -5 - 0.1 (outside the nave), X anywhere near along-length
            let x  = -NAVE_L / 2.0 + 1.0 + hash2(i, 3) * (NAVE_L - 2.0);
            let z  = -NAVE_W / 2.0 - 0.1 + (hash2(i, 4) - 0.5) * 2.0; // ±1 m outside/inside
            (x, z)
        } else if pick < 0.80 {
            // WEST WALL BASE — X = -8 - 0.1, Z anywhere except tower-covered region (Z>=-6,-1)
            let mut z = -NAVE_W / 2.0 + 1.0 + hash2(i, 5) * (NAVE_W - 2.0);
            // if it would land on the tower footprint (Z ∈ [-6,-1]), bias it north
            if z >= -NAVE_W / 2.0 - 1.0 && z <= -NAVE_W / 2.0 + TOWER_W - 1.0 { z += 2.5; }
            let x = -NAVE_L / 2.0 - 0.1 + (hash2(i, 6) - 0.5) * 2.0;
            (x, z)
        } else if pick < 0.90 {
            // NORTH WALL BASE — Z = +5 + 0.1, X mid-length
            let x = -NAVE_L / 2.0 + 3.0 + hash2(i, 8) * (NAVE_L - 6.0);
            let z =  NAVE_W / 2.0 + 0.1 + (hash2(i, 9) - 0.5) * 1.5;
            (x, z)
        } else {
            // EAST WALL BASE — X = +8 + 0.1, Z mid (but outside apse so |Z| ≤ 4 only)
            let z = -APSE_R * 0.5 + hash2(i, 10) * APSE_R;
            let x =  NAVE_L / 2.0 + 0.1 + (hash2(i, 11) - 0.5) * 1.5;
            (x, z)
        };
        let s = 0.15 + hash2(i, 12) * 0.25;
        let mut m = cube(s);
        m = m.translated_by(Vec3::new(x, s * 0.4, z));
        m = m.rotated_by(Quat::from_euler(EulerRot::XYZ, hash2(i, 1) * 3.0, 0.0, hash2(i, 2) * 3.0));
        let c = if hash2(i, 13) < 0.4 { BRICK } else { STONE_DARK };
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