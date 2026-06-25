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
// Ported from: src/lib/subs/pathfind.c
// Known contributors to the original:
//    Dave Pare, 1989

// BFS pathfinder on the Empire toroidal hex grid.
//
// The Empire map is a toroidal hex grid where neighbors are reached
// via the six direction offsets (DIR_FIRST..=DIR_LAST) from geo.rs.
//
// This module provides `find_path`, which performs a BFS from source to
// destination and returns the sequence of direction indices (1-6 matching
// DIR_UR..=DIR_UL) that form the shortest passable path.

use std::collections::{HashMap, VecDeque};
use empire_types::coords::Coord;
use super::geo::{DIROFF, DIR_FIRST, DIR_LAST, x_norm, y_norm};

/// Find the shortest path from `(sx, sy)` to `(dx, dy)` on the toroidal hex
/// grid, returning the sequence of direction indices (1–6) corresponding to
/// `DIR_UR..=DIR_UL` in geo.rs.
///
/// The `passable` closure returns `true` for sectors the moving unit can enter.
/// The source sector is never checked for passability; the destination is.
///
/// Returns an empty `Vec` if no path exists.
pub fn find_path(
    sx: Coord,
    sy: Coord,
    dx: Coord,
    dy: Coord,
    world_x: i32,
    world_y: i32,
    passable: impl Fn(Coord, Coord) -> bool,
) -> Vec<u8> {
    // Trivial case: already at destination
    if sx == dx && sy == dy {
        return Vec::new();
    }

    // BFS state: map from (x, y) -> (direction taken to arrive here, parent coords)
    // We pack (x, y) as a single i32 key for efficiency.
    let pack = |x: Coord, y: Coord| -> i32 { (x as i32) * 10_000 + (y as i32) };

    let src_key = pack(sx, sy);
    let dst_key = pack(dx, dy);

    // parent[(x,y)] = (prev_x, prev_y, dir_index that moved from prev to (x,y))
    let mut parent: HashMap<i32, (Coord, Coord, u8)> = HashMap::new();
    let mut queue: VecDeque<(Coord, Coord)> = VecDeque::new();

    parent.insert(src_key, (sx, sy, 0)); // sentinel — source has no parent
    queue.push_back((sx, sy));

    while let Some((cx, cy)) = queue.pop_front() {
        if pack(cx, cy) == dst_key {
            // Reconstruct path by walking parent chain from dst back to src
            let mut path: Vec<u8> = Vec::new();
            let mut cur_key = dst_key;
            loop {
                let &(px, py, dir) = parent.get(&cur_key).expect("BFS parent invariant");
                if pack(px, py) == src_key && dir == 0 {
                    break; // reached the source sentinel
                }
                path.push(dir);
                cur_key = pack(px, py);
                if cur_key == src_key {
                    break;
                }
            }
            path.reverse();
            return path;
        }

        for dir_idx in DIR_FIRST..=DIR_LAST {
            let (ddx, ddy) = DIROFF[dir_idx];
            let nx = x_norm(cx + ddx, world_x);
            let ny = y_norm(cy + ddy, world_y);
            let nk = pack(nx, ny);

            if parent.contains_key(&nk) {
                continue; // already visited
            }
            if !passable(nx, ny) {
                continue;
            }

            parent.insert(nk, (cx, cy, dir_idx as u8));
            queue.push_back((nx, ny));
        }
    }

    // No path found
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tiny 8x8 open ocean world — everything passable.
    fn open(_x: Coord, _y: Coord) -> bool { true }

    #[test]
    fn trivial_same_sector() {
        let path = find_path(0, 0, 0, 0, 64, 32, open);
        assert!(path.is_empty());
    }

    #[test]
    fn single_step_ur() {
        // DIR_UR = index 1, offset (1, -1) → from (0,0) goes to (1, 31) on 64x32 world
        let path = find_path(0, 0, 1, 31, 64, 32, open);
        assert_eq!(path.len(), 1);
        assert_eq!(path[0], 1); // DIR_UR
    }

    #[test]
    fn blocked_path_returns_empty() {
        // Block all neighbors of source
        let blocked = |x: Coord, y: Coord| -> bool { x == 0 && y == 0 };
        let path = find_path(0, 0, 2, 0, 64, 32, blocked);
        assert!(path.is_empty());
    }
}
