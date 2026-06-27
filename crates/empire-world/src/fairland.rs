// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// Empire is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/util/fairland.c
// Known contributors to the original:
//    Ken Stevens, 1995
//    Steve McClure, 1998
//    Markus Armbruster, 2004-2020

//! Fairland world generator — creates sector map from scratch.
//!
//! Algorithm (5 phases):
//! 1. Drift capitals to maximise mutual distance (perturbation technique).
//! 2. Grow start islands ("continents") from capital positions.
//! 3. Place & grow additional islands, one per sphere of influence.
//! 4. Create elevations via random walk, then normalise land/sea separately.
//! 5. Compute sector resources from elevation curves.

use rand::prelude::*;
use empire_types::sector::{Sector, SectorType, DistEntry};
use empire_types::commodity::Inventory;
use empire_types::coords::{Coord, NatId};

// ── Direction constants ───────────────────────────────────────────────────────

const DIR_STOP:  usize = 0;
const DIR_UR:    usize = 1;
const DIR_R:     usize = 2;
const DIR_DR:    usize = 3;
const DIR_DL:    usize = 4;
const DIR_L:     usize = 5;
const DIR_UL:    usize = 6;
const DIR_FIRST: usize = DIR_UR;
const DIR_LAST:  usize = DIR_UL;

/// (dx, dy) for each direction.  Indexed by DIR_*.
/// Must match dir.c diroff[] in Empire 4.
const DIROFF: [(i32, i32); 7] = [
    ( 0,  0), // stop
    ( 1, -1), // UR
    ( 2,  0), // R
    ( 1,  1), // DR
    (-1,  1), // DL
    (-2,  0), // L
    (-1, -1), // UL
];

fn dir_back(d: usize) -> usize {
    if d <= DIR_DR { d + 3 } else { d - 3 }
}

// ── Elevation constants ───────────────────────────────────────────────────────

const LANDMIN:  i32 = 1;    // minimum elevation for normal land
const PLATMIN:  i32 = 36;   // elevation for capital plateau sectors
const HIGHMIN:  i32 = 98;   // minimum elevation for mountains

// ── Algorithm constants ───────────────────────────────────────────────────────

pub const NUMTRIES:          i32   = 10;
const STABLE_CYCLE:          usize = 4;
const ALL_LAND_BITS:         u8    = 0b0111_1110; // bits DIR_FIRST..=DIR_LAST

// ── Resource interpolation tables ────────────────────────────────────────────
// Each table is a sequence of (elevation, resource_value) break-points.
// Values between break-points are linearly interpolated.

struct Pt { elev: i32, res: i32 }

const IRON_CONF: &[Pt] = &[
    Pt { elev: -127, res:   0 }, Pt { elev:  21, res:   0 },
    Pt { elev:   85, res: 100 }, Pt { elev:  97, res: 100 },
    Pt { elev:   98, res:   0 }, Pt { elev: 127, res:   0 },
];
const GOLD_CONF: &[Pt] = &[
    Pt { elev: -127, res:  0 }, Pt { elev:  35, res:  0 },
    Pt { elev:   97, res: 80 }, Pt { elev:  98, res: 80 },
    Pt { elev:  127, res: 85 },
];
const FERT_CONF: &[Pt] = &[
    Pt { elev: -127, res: 100 }, Pt { elev: -59, res: 100 },
    Pt { elev:    0, res:  41 }, Pt { elev:   1, res: 100 },
    Pt { elev:   10, res: 100 }, Pt { elev:  56, res:   0 },
    Pt { elev:  127, res:   0 },
];
const OIL_CONF: &[Pt] = &[
    Pt { elev: -127, res: 100 }, Pt { elev: -49, res: 100 },
    Pt { elev:    0, res:   2 }, Pt { elev:   1, res: 100 },
    Pt { elev:    6, res: 100 }, Pt { elev:  34, res:   0 },
    Pt { elev:  127, res:   0 },
];
const URAN_CONF: &[Pt] = &[
    Pt { elev: -127, res:   0 }, Pt { elev:  55, res:   0 },
    Pt { elev:   90, res: 100 }, Pt { elev:  97, res: 100 },
    Pt { elev:   98, res:   0 }, Pt { elev: 127, res:   0 },
];

fn elev_to_resource(elev: i32, conf: &[Pt]) -> u8 {
    let i = conf.partition_point(|p| p.elev < elev).saturating_sub(1);
    let i = i.min(conf.len() - 2);
    let e1 = conf[i].elev;
    let e2 = conf[i + 1].elev;
    let r1 = conf[i].res;
    let r2 = conf[i + 1].res;
    if e2 == e1 { return r1.clamp(0, 100) as u8; }
    let v = r1 + (elev - e1) * (r2 - r1) / (e2 - e1);
    v.clamp(0, 100) as u8
}

// ── Hexagon iterator ──────────────────────────────────────────────────────────

/// Iterates around a hex ring of radius `n` centred on the start position.
struct HexagonIter { dir: usize, step: usize, n: usize }

impl HexagonIter {
    fn new(n: usize) -> Self { HexagonIter { dir: DIR_FIRST, step: 0, n } }

    /// Move one step around the ring.  Returns false when the ring is complete.
    fn advance(&mut self, x: &mut usize, y: &mut usize, wx: usize, wy: usize) -> bool {
        let (dx, dy) = DIROFF[self.dir];
        *x = nx(*x as i32 + dx, wx);
        *y = ny(*y as i32 + dy, wy);
        self.step += 1;
        if self.step == self.n { self.step = 0; self.dir += 1; }
        self.dir <= DIR_LAST
    }
}

// ── Coordinate helpers ────────────────────────────────────────────────────────

fn nx(x: i32, wx: usize) -> usize { ((x % wx as i32 + wx as i32) % wx as i32) as usize }
fn ny(y: i32, wy: usize) -> usize { ((y % wy as i32 + wy as i32) % wy as i32) as usize }
fn off(x: usize, y: usize, wx: usize) -> usize { y * wx + x }

fn map_dist(x1: usize, y1: usize, x2: usize, y2: usize, wx: usize, wy: usize) -> i32 {
    let mut dx = (x1 as i32 - x2 as i32).abs() % wx as i32;
    if dx > wx as i32 / 2 { dx = wx as i32 - dx; }
    let mut dy = (y1 as i32 - y2 as i32).abs() % wy as i32;
    if dy > wy as i32 / 2 { dy = wy as i32 - dy; }
    if dx > dy { (dx - dy) / 2 + dy } else { dy }
}

// ── Main struct ───────────────────────────────────────────────────────────────

pub struct Fairland {
    // Parameters
    pub world_x: usize,
    pub world_y: usize,
    pub nc: usize,          // number of continents
    pub sc: usize,          // continent size (sectors)
    pub ni: usize,          // number of islands
    pub is: usize,          // average island size
    pub sp: i32,            // spike percentage (0 = round, 100 = snake)
    pub pm: i32,            // mountain percentage
    pub di: i32,            // min distance between continents
    pub id: i32,            // min distance from islands to continents
    pub distinct_islands: bool,
    pub quiet: bool,

    rng: StdRng,

    // Per-sector arrays (length = world_x * world_y; only valid at (x+y)%2==0)
    own:      Vec<i32>,   // -1 = sea, ≥0 = island/continent index
    adj_land: Vec<u8>,    // bitmask of adjacent land directions (bits 1..=6)
    elev:     Vec<i32>,   // elevation
    xzone:    Vec<i32>,   // exclusive zone: -1=free, -2=contested, ≥0=owner
    seen:     Vec<u32>,   // BFS generation tag
    closest:  Vec<i32>,   // closest continent (-1 = contested)
    distance: Vec<u32>,   // distance to closest continent's coast

    bfs_head: usize,
    bfs_tail: usize,
    bfs_buf:  Vec<usize>, // ring buffer for BFS queue

    cur_seen: u32,

    // Per-island
    cap:   Vec<(usize, usize)>,        // capital coordinates
    sect:  Vec<Vec<(usize, usize)>>,   // sector list per island
    isecs: Vec<usize>,                 // sector count per island
}

impl Fairland {
    pub fn new(
        world_x: usize, world_y: usize,
        nc: usize, sc: usize,
        ni: usize, is: usize,
        sp: i32, pm: i32, di: i32, id: i32,
        distinct_islands: bool,
        quiet: bool,
        seed: u64,
    ) -> Self {
        let sz = world_x * world_y;
        let total = nc + ni;

        let mut sect = Vec::with_capacity(total);
        for i in 0..total {
            let cap = if i < nc { sc } else { is * 2 };
            sect.push(Vec::with_capacity(cap));
        }

        Fairland {
            world_x, world_y, nc, sc, ni, is, sp, pm, di, id,
            distinct_islands, quiet,
            rng: StdRng::seed_from_u64(seed),
            own:      vec![-1i32; sz],
            adj_land: vec![0u8;   sz],
            elev:     vec![0i32;  sz],
            xzone:    vec![-1i32; sz],
            seen:     vec![0u32;  sz],
            closest:  vec![-1i32; sz],
            distance: vec![u32::MAX; sz],
            bfs_head: 0, bfs_tail: 0, bfs_buf: vec![0usize; sz + 1],
            cur_seen: 0,
            cap:   vec![(0, 0); nc],
            sect,
            isecs: vec![0usize; total],
        }
    }

    fn roll0(&mut self, n: i32) -> i32 {
        if n <= 0 { return 0; }
        self.rng.gen_range(0..n)
    }

    fn sz(&self) -> usize { self.world_x * self.world_y }

    fn off(&self, x: usize, y: usize) -> usize { off(x, y, self.world_x) }

    fn nx(&self, x: i32) -> usize { nx(x, self.world_x) }
    fn ny(&self, y: i32) -> usize { ny(y, self.world_y) }

    fn dist(&self, x1: usize, y1: usize, x2: usize, y2: usize) -> i32 {
        map_dist(x1, y1, x2, y2, self.world_x, self.world_y)
    }

    fn is_coastal(&self, x: usize, y: usize) -> bool {
        self.adj_land[self.off(x, y)] != ALL_LAND_BITS
    }

    fn qprint(&self, msg: &str) {
        if !self.quiet { print!("{}", msg); }
    }

    // ── Init ─────────────────────────────────────────────────────────────────

    fn init(&mut self) {
        for v in self.own.iter_mut()      { *v = -1; }
        for v in self.adj_land.iter_mut() { *v = 0; }
        for v in self.elev.iter_mut()     { *v = 0; }
        for sect in self.sect.iter_mut()  { sect.clear(); }
        for v in self.isecs.iter_mut()    { *v = 0; }
        self.cur_seen = 0;
    }

    // ── Drift (capital placement) ─────────────────────────────────────────────

    fn iso(&self, j: usize, nx: usize, ny: usize) -> i32 {
        let mut d = i32::MAX;
        for i in 0..self.nc {
            if i == j { continue; }
            let md = self.dist(self.cap[i].0, self.cap[i].1, nx, ny);
            if md < d { d = md; }
        }
        d
    }

    fn drift_capital(&mut self, j: usize) {
        let start_dir = (DIR_L + self.roll0(6) as usize) % 7;
        let cx = self.cap[j].0;
        let cy = self.cap[j].1;
        let base_iso = self.iso(j, cx, cy);
        let mut dir = start_dir;
        for _ in 0..6 {
            if dir < DIR_FIRST || dir > DIR_LAST {
                dir = DIR_FIRST;
            }
            let (dx, dy) = DIROFF[dir];
            let nx2 = self.nx(cx as i32 + dx);
            let ny2 = self.ny(cy as i32 + dy);
            if self.iso(j, nx2, ny2) >= base_iso {
                self.cap[j] = (nx2, ny2);
                return;
            }
            dir = if dir >= DIR_LAST { DIR_FIRST } else { dir + 1 };
        }
    }

    fn stable(&mut self, turns: usize, mc: &mut [i32; 4]) -> bool {
        let before_check = (self.world_x + self.world_y) / 2;
        if turns == 0 {
            for (i, v) in mc.iter_mut().enumerate() { *v = i as i32; }
        }
        if turns <= before_check { return false; }

        let mut d = 0i32;
        for i in 0..self.nc {
            let (cx, cy) = self.cap[i];
            let isod = self.iso(i, cx, cy);
            if isod > d { d = isod; }
        }
        let mut stab = true;
        for v in mc.iter() { if *v != d { stab = false; } }
        mc[turns % STABLE_CYCLE] = d;
        stab
    }

    pub fn drift(&mut self) -> bool {
        let drift_max = (self.world_x + self.world_y) * 2;

        // Place capitals evenly across world
        for i in 0..self.nc {
            let y = (2 * i) / self.world_x;
            let x = (2 * i) % self.world_x + y % 2;
            self.cap[i] = (x, y);
        }

        let mut mc = [0i32; STABLE_CYCLE];
        for turns in 0..drift_max {
            if self.stable(turns, &mut mc) { return true; }
            for i in 0..self.nc {
                self.drift_capital(i);
            }
        }
        false
    }

    // ── Exclusive zones ───────────────────────────────────────────────────────

    fn xzone_ok(&self, c: usize, x: usize, y: usize) -> bool {
        let v = self.xzone[self.off(x, y)];
        v == c as i32 || v == -1
    }

    fn xzone_around_sector(&mut self, c: usize, x: usize, y: usize, dist: i32) {
        let o = self.off(x, y);
        self.xzone[o] = c as i32;

        for d in 1..=dist as usize {
            let mut rx = self.nx(x as i32 - 2 * d as i32);
            let mut ry = y;
            let mut iter = HexagonIter::new(d);
            loop {
                let ro = self.off(rx, ry);
                if self.xzone[ro] == -1 {
                    self.xzone[ro] = c as i32;
                } else if self.xzone[ro] != c as i32 {
                    self.xzone[ro] = -2;
                }
                if !iter.advance(&mut rx, &mut ry, self.world_x, self.world_y) { break; }
            }
        }
    }

    fn xzone_around_island(&mut self, c: usize, dist: i32) {
        let secs: Vec<(usize, usize)> = self.sect[c].clone();
        for (x, y) in secs {
            self.xzone_around_sector(c, x, y, dist);
        }
    }

    fn xzone_init(&mut self, n: usize) {
        for v in self.xzone.iter_mut() { *v = -1; }
        for c in 0..n {
            let dist = self.id;
            self.xzone_around_island(c, dist);
        }
    }

    // ── BFS ──────────────────────────────────────────────────────────────────

    fn bfs_init(&mut self) {
        for v in self.closest.iter_mut()  { *v = -1; }
        for v in self.distance.iter_mut() { *v = u32::MAX; }
        self.bfs_head = 0;
        self.bfs_tail = 0;
    }

    fn bfs_enqueue(&mut self, c: i32, x: usize, y: usize, dist: u32) {
        let o = self.off(x, y);
        self.closest[o]  = c;
        self.distance[o] = dist;
        self.bfs_buf[self.bfs_tail] = o;
        self.bfs_tail += 1;
        if self.bfs_tail >= self.bfs_buf.len() { self.bfs_tail = 0; }
    }

    fn bfs_run_queue(&mut self) {
        let wx = self.world_x;
        let wy = self.world_y;
        while self.bfs_head != self.bfs_tail {
            let o = self.bfs_buf[self.bfs_head];
            self.bfs_head += 1;
            if self.bfs_head >= self.bfs_buf.len() { self.bfs_head = 0; }

            let dist = self.distance[o] + 1;
            let x = o % wx;
            let y = o / wx;

            for dir in DIR_FIRST..=DIR_LAST {
                let (dx, dy) = DIROFF[dir];
                let nx2 = nx(x as i32 + dx, wx);
                let ny2 = ny(y as i32 + dy, wy);
                let no = off(nx2, ny2, wx);
                if dist < self.distance[no] {
                    let c = self.closest[o];
                    self.bfs_enqueue(c, nx2, ny2, dist);
                } else if self.distance[no] == dist {
                    if self.closest[o] != self.closest[no] {
                        self.closest[no] = -1;
                    }
                }
            }
        }
    }

    fn bfs_enqueue_island(&mut self, c: usize) {
        let secs: Vec<(usize, usize)> = self.sect[c].clone();
        for (x, y) in secs {
            if self.is_coastal(x, y) {
                let dist = self.distance[self.off(x, y)];
                if dist > 0 {
                    self.bfs_enqueue(c as i32, x, y, 0);
                }
            }
        }
    }

    fn bfs_enqueue_border(&mut self) {
        let wx = self.world_x;
        let wy = self.world_y;
        let id = self.id as u32;
        let mut borders: Vec<(i32, usize, usize, u32)> = Vec::new();

        for y in 0..wy {
            for x in (y % 2..wx).step_by(2) {
                let o = off(x, y, wx);
                if self.distance[o] <= id + 1 { continue; }
                if self.closest[o] == -1 { continue; }
                let c = self.closest[o];
                let mut is_border = false;
                for dir in DIR_FIRST..=DIR_LAST {
                    let (dx, dy) = DIROFF[dir];
                    let nx2 = nx(x as i32 + dx, wx);
                    let ny2 = ny(y as i32 + dy, wy);
                    let no = off(nx2, ny2, wx);
                    if self.closest[no] != c {
                        is_border = true;
                        break;
                    }
                }
                if is_border {
                    borders.push((c, x, y, id + 1));
                }
            }
        }
        for (c, x, y, dist) in borders {
            if dist < self.distance[self.off(x, y)] {
                self.bfs_enqueue(c, x, y, dist);
            }
        }
    }

    fn init_spheres_of_influence(&mut self) {
        self.bfs_init();
        for c in 0..self.nc {
            self.bfs_enqueue_island(c);
        }
        self.bfs_run_queue();
        self.bfs_enqueue_border();
        self.bfs_run_queue();
    }

    fn init_distance_to_coast(&mut self) {
        let total = self.nc + self.ni;
        self.bfs_init();
        for c in 0..total {
            self.bfs_enqueue_island(c);
        }
        self.bfs_run_queue();
    }

    // ── Growing ───────────────────────────────────────────────────────────────

    fn is_in_sphere(&self, c: usize, x: usize, y: usize) -> bool {
        if c < self.nc { return true; }
        self.closest[self.off(x, y)] == (c % self.nc) as i32
    }

    fn can_grow_at(&self, c: usize, x: usize, y: usize) -> bool {
        self.own[self.off(x, y)] == -1
            && self.xzone_ok(c, x, y)
            && self.is_in_sphere(c, x, y)
    }

    fn adj_land_update(&mut self, x: usize, y: usize) {
        let is_land = self.own[self.off(x, y)] != -1;
        for dir in DIR_FIRST..=DIR_LAST {
            let (dx, dy) = DIROFF[dir];
            let nx2 = self.nx(x as i32 + dx);
            let ny2 = self.ny(y as i32 + dy);
            let no = self.off(nx2, ny2);
            let bit = 1u8 << dir_back(dir);
            if is_land {
                self.adj_land[no] |= bit;
            } else {
                self.adj_land[no] &= !bit;
            }
        }
    }

    fn add_sector(&mut self, c: usize, x: usize, y: usize) {
        let o = self.off(x, y);
        let xzone_dist = if c < self.nc { self.di } else if self.distinct_islands { self.id } else { 0 };
        self.xzone_around_sector(c, x, y, xzone_dist);
        self.sect[c].push((x, y));
        self.isecs[c] += 1;
        self.own[o] = c as i32;
        self.adj_land_update(x, y);
    }

    fn grow_weight(&self, x: usize, y: usize, spike: bool) -> i32 {
        let bits = self.adj_land[self.off(x, y)];
        let n = bits.count_ones() as i32;
        if n == 0 || n >= 7 { return 0; }
        if spike { let k = 6 - n; k * k } else { n * n * n }
    }

    fn grow_one_sector(&mut self, c: usize) -> bool {
        let spike = self.roll0(100) < self.sp;

        debug_assert!(self.cur_seen < u32::MAX);
        self.cur_seen += 1;
        let tag = self.cur_seen;

        let mut wsum = 0i32;
        let mut best: Option<(usize, usize)> = None;

        let nsecs = self.isecs[c];
        for i in 0..nsecs {
            let (x, y) = self.sect[c][i];
            let o = self.off(x, y);

            for dir in DIR_FIRST..=DIR_LAST {
                if self.adj_land[o] & (1u8 << dir) != 0 { continue; }
                let nx2 = self.nx(x as i32 + DIROFF[dir].0);
                let ny2 = self.ny(y as i32 + DIROFF[dir].1);
                let no = self.off(nx2, ny2);
                if self.seen[no] == tag { continue; }
                self.seen[no] = tag;
                if !self.can_grow_at(c, nx2, ny2) { continue; }
                let w = self.grow_weight(nx2, ny2, spike);
                if w == 0 { continue; }
                wsum += w;
                if self.roll0(wsum) < w {
                    best = Some((nx2, ny2));
                }
            }
        }

        if let Some((bx, by)) = best {
            self.add_sector(c, bx, by);
            true
        } else {
            false
        }
    }

    pub fn grow_continents(&mut self) -> bool {
        self.xzone_init(0);

        let mut done = true;
        for c in 0..self.nc {
            self.isecs[c] = 0;
            let (cx, cy) = self.cap[c];
            let cx2 = self.nx(cx as i32 + 2);
            if !self.can_grow_at(c, cx, cy) || !self.can_grow_at(c, cx2, cy) {
                done = false;
                continue;
            }
            self.add_sector(c, cx, cy);
            self.add_sector(c, cx2, cy);
        }
        if !done {
            self.qprint("No room for continents\n");
            return false;
        }

        let mut secs = 2usize;
        while secs < self.sc && done {
            for c in 0..self.nc {
                if !self.grow_one_sector(c) { done = false; }
            }
            secs += 1;
        }
        if !done {
            self.qprint(&format!("Only managed to grow {} out of {} sectors.\n", secs - 1, self.sc));
        }
        done
    }

    fn place_island(&mut self, c: usize, isiz: usize) -> bool {
        let wx = self.world_x;
        let wy = self.world_y;
        let id = self.id as u32;

        let mut n = 0i32;
        let mut best: Option<(usize, usize)> = None;

        for y in 0..wy {
            for x in (y % 2..wx).step_by(2) {
                if !self.can_grow_at(c, x, y) { continue; }
                let d = self.distance[self.off(x, y)];
                if d <= id { continue; }
                let gap = (d - id) as i32;
                let w = gap * gap;
                let cap = ((isiz + 2) / 3) as i32;
                n += w.min(cap);
                if self.roll0(n) < w {
                    best = Some((x, y));
                }
            }
        }

        if let Some((bx, by)) = best {
            self.add_sector(c, bx, by);
            true
        } else {
            false
        }
    }

    fn size_islands(&mut self) -> Vec<usize> {
        let n = if self.nc == 0 { 0 } else { self.ni / self.nc };
        if n == 0 { return vec![]; }
        let mut isiz = vec![0i32; n];
        isiz[0] = (n * self.is) as i32;
        let mut r1 = self.roll0(self.is as i32);
        for i in 1..n {
            let r0 = r1;
            r1 = self.roll0(self.is as i32);
            isiz[i] = self.is as i32 + r1 - r0;
            isiz[0] -= isiz[i];
        }
        isiz.sort_unstable_by(|a, b| b.cmp(a));
        isiz.iter().map(|&s| s.max(1) as usize).collect()
    }

    pub fn grow_islands(&mut self) -> bool {
        let island_size = self.size_islands();
        let n = island_size.len();
        if n == 0 { return true; }

        self.init_spheres_of_influence();

        let mut xzone_valid = false;
        let mut carry: i32 = 0;

        for i in 0..n {
            let c = self.nc + i * self.nc;

            if !xzone_valid {
                self.xzone_init(c);
                xzone_valid = true;
            }

            carry += island_size[i] as i32;
            let isiz = (2 * self.is as i32).min(carry) as usize;

            for j in 0..self.nc {
                self.isecs[c + j] = 0;
                if !self.place_island(c + j, isiz) {
                    self.qprint(&format!("\nNo room for island #{}\n", c - self.nc + j + 1));
                    return false;
                }
            }

            let mut done = true;
            let mut secs = 1usize;
            while secs < isiz && done {
                for j in 0..self.nc {
                    if !self.grow_one_sector(c + j) { done = false; }
                }
                secs += 1;
            }

            if !done {
                secs -= 1;
                for j in 0..self.nc {
                    if self.isecs[c + j] != secs {
                        let last_idx = self.isecs[c + j] - 1;
                        let (lx, ly) = self.sect[c + j][last_idx];
                        let lo = self.off(lx, ly);
                        self.own[lo] = -1;
                        self.sect[c + j].pop();
                        self.isecs[c + j] -= 1;
                        self.adj_land_update(lx, ly);
                    }
                }
                xzone_valid = false;
            }

            if !self.quiet {
                for j in 0..self.nc {
                    print!(" {}({})", c - self.nc + j + 1, self.isecs[c + j]);
                }
            }

            carry -= secs as i32;
        }

        if !self.quiet {
            println!();
            let total_is = self.is * self.ni;
            if carry > 0 {
                println!(
                    "Only managed to grow {} out of {} island sectors.",
                    total_is as i32 - carry * self.nc as i32,
                    total_is
                );
            }
        }

        true
    }

    // ── Elevation ─────────────────────────────────────────────────────────────

    fn elevate_prep(&mut self) {
        self.init_distance_to_coast();
        let iterations = self.sz() * 8;
        let wx = self.world_x;
        let wy = self.world_y;

        for _ in 0..iterations {
            let o0 = self.roll0(self.sz() as i32) as usize;
            let x0 = o0 % wx;
            let y0 = o0 / wx;
            let (r, sign) = if self.own[o0] == -1 {
                let r = self.roll0(3.min(self.distance[o0] as i32)) + 1;
                (r, -1i32)
            } else {
                let r = self.roll0(3.min(self.distance[o0] as i32) + 1) + 1;
                (r, 1i32)
            };
            let new_e = self.elev[o0] + sign * r * r;
            self.elev[o0] = new_e.clamp(i16::MIN as i32, i16::MAX as i32);

            for d in 1..r as usize {
                let mut rx = self.nx(x0 as i32 - 2 * d as i32);
                let mut ry = y0;
                let mut iter = HexagonIter::new(d);
                loop {
                    let ro = off(rx, ry, wx);
                    let delta = sign * (r * r - d as i32 * d as i32);
                    let ne = self.elev[ro] + delta;
                    self.elev[ro] = ne.clamp(i16::MIN as i32, i16::MAX as i32);
                    if !iter.advance(&mut rx, &mut ry, wx, wy) { break; }
                }
            }
        }
    }

    fn elevate_land(&mut self) {
        let max_size = self.sc.max(self.is * 2);
        let max_nm = (self.pm * max_size as i32) / 100;
        let total = self.nc + self.ni;

        for c in 0..total {
            let nm = (self.pm * self.isecs[c] as i32) / 100;
            let i0 = if c < self.nc { 2 } else { 0 };
            let n = self.isecs[c].saturating_sub(i0);

            // Set plateau elevation for capital sectors
            for i in 0..i0 {
                let (x, y) = self.sect[c][i];
                let o = self.off(x, y);
                self.elev[o] = PLATMIN;
            }

            if n == 0 { continue; }

            // Collect offsets and sort by elevation
            let mut offs: Vec<usize> = (i0..self.isecs[c])
                .map(|i| { let (x,y) = self.sect[c][i]; self.off(x, y) })
                .collect();
            offs.sort_unstable_by(|&a, &b| {
                self.elev[a].cmp(&self.elev[b]).then(a.cmp(&b))
            });

            // Non-mountain: interpolate from LANDMIN to HIGHMIN-1
            let non_mount = n.saturating_sub(nm as usize);
            if non_mount > 0 {
                let delta = (HIGHMIN - LANDMIN - 1) as f64 / non_mount.max(1) as f64;
                let mut elevation = LANDMIN as f64;
                for i in 0..non_mount {
                    self.elev[offs[i]] = (elevation + 0.5) as i32;
                    elevation += delta;
                }
            }

            // Mountains: interpolate from HIGHMIN to 127
            let mountain_delta = (127.0 - HIGHMIN as f64) / (max_nm.max(1)) as f64;
            let mut elevation = HIGHMIN as f64;
            for i in non_mount..n {
                elevation += mountain_delta;
                self.elev[offs[i]] = (elevation + 0.5) as i32;
            }
        }
    }

    fn elevate_sea(&mut self) {
        let min = self.elev.iter().cloned().filter(|&e| e < 0).fold(0i32, i32::min);
        if min >= 0 { return; }
        for e in self.elev.iter_mut() {
            if *e < 0 {
                *e = -1 - 126 * *e / min;
            }
        }
    }

    pub fn create_elevations(&mut self) {
        self.elevate_prep();
        self.elevate_land();
        self.elevate_sea();
    }

    // ── Sector type / resources ───────────────────────────────────────────────

    fn elev_to_sct_type(e: i32) -> SectorType {
        if e < LANDMIN { SectorType::Sea }
        else if e < HIGHMIN { SectorType::Wilderness }
        else { SectorType::Mountain }
    }

    // ── Output ────────────────────────────────────────────────────────────────

    pub fn print_map(&self) {
        let wx = self.world_x as i32;
        let wy = self.world_y as i32;
        for sy in -(wy / 2)..(wy / 2) {
            let y = ((sy % wy + wy) % wy) as usize;
            println!();
            if y % 2 != 0 { print!(" "); }
            let start_x = (-(wx / 2) + y as i32 % 2) as i32;
            let mut sx = start_x;
            while sx < wx / 2 {
                let x = ((sx % wx + wx) % wx) as usize;
                let o = self.off(x, y);
                let c = self.own[o];
                let t = Self::elev_to_sct_type(self.elev[o]);
                let ch = match t {
                    SectorType::Sea      => '.',
                    SectorType::Mountain => '^',
                    _ if c < 0 => '.',               // sea sector elevated by prep spillover
                    _ if c >= self.nc as i32 => '%',
                    _ => {
                        let cc = c as usize;
                        let is_cap = (x == self.cap[cc].0
                            || x == self.nx(self.cap[cc].0 as i32 + 2))
                            && y == self.cap[cc].1;
                        if is_cap { NUMLETTER[cc % 62] } else { '#' }
                    }
                };
                print!("{} ", ch);
                sx += 2;
            }
        }
        println!();
    }

    /// Produce the full sector list for writing to the DB.
    pub fn build_sectors(&self) -> Vec<Sector> {
        let wx = self.world_x;
        let wy = self.world_y;
        let mut out = Vec::with_capacity(wx * wy / 2 + 1);

        for y in 0..wy {
            for x in (y % 2..wx).step_by(2) {
                let o = self.off(x, y);
                let e = self.elev[o];
                let st = Self::elev_to_sct_type(e);
                let coastal = if st == SectorType::Sea { false } else { self.is_coastal(x, y) };
                let dterr = if self.own[o] >= 0 { (self.own[o] + 1) as u8 } else { 0 };

                out.push(Sector {
                    uid:         o as i32,
                    own:         0,
                    x:           x as Coord,
                    y:           y as Coord,
                    sector_type: st,
                    effic:       0,
                    mobil:       0,
                    off:         false,
                    loyal:       0,
                    terr:        [0; 4],
                    dterr,
                    dist_x:      0,
                    dist_y:      0,
                    avail:       0,
                    flags:       0,
                    elev:        e.clamp(-128, 127) as i16,
                    work:        100,
                    coastal,
                    new_type:    st,
                    min:         elev_to_resource(e, IRON_CONF),
                    gmin:        elev_to_resource(e, GOLD_CONF),
                    fertil:      elev_to_resource(e, FERT_CONF),
                    oil:         elev_to_resource(e, OIL_CONF),
                    uran:        elev_to_resource(e, URAN_CONF),
                    old_own:     0,
                    che:         0,
                    che_target:  0 as NatId,
                    items:       Inventory::default(),
                    del:         [DistEntry::default(); 26],
                    mines:       0,
                    pstage:      0,
                    ptime:       0,
                    fallout:     0,
                });
            }
        }
        out
    }

    /// Capital coordinates (world-absolute, 0-based), one per continent.
    pub fn capitals(&self) -> &[(usize, usize)] { &self.cap[..self.nc] }
}

// ── Static helpers ────────────────────────────────────────────────────────────

const NUMLETTER: [char; 62] = [
    '0','1','2','3','4','5','6','7','8','9',
    'a','b','c','d','e','f','g','h','i','j','k','l','m',
    'n','o','p','q','r','s','t','u','v','w','x','y','z',
    'A','B','C','D','E','F','G','H','I','J','K','L','M',
    'N','O','P','Q','R','S','T','U','V','W','X','Y','Z',
];
