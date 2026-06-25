# Empire 5 — Rust Rewrite of Wolfpack Empire

Empire 5 is a ground-up rewrite of [Wolfpack Empire 4.4.1](../empire4.4.1/) in Rust. The reference C server remains at `../empire4.4.1/` and continues to run as the functional baseline. Every feature of the original game is preserved; only the implementation language and internal architecture change.

## What Empire Is

Empire is a text-based multiplayer strategy wargame dating to 1986. Players connect over TCP, each managing a nation on a shared map: building sectors, training armies, constructing navies, researching technology, waging war, and pursuing diplomacy. The server runs continuously; an economic/military update fires on a configurable schedule (ETU — Empire Time Units).

## Why Rust

| Concern | C (4.4.1) | Rust (5.x) |
|---|---|---|
| Memory safety | None — manual malloc/free, fixed char arrays | Compile-time guaranteed — no buffer overflows |
| Integer safety | Silent overflow, UB on signed wrap | Explicit checked/wrapping/saturating arithmetic |
| Concurrency | Custom cooperative LWP threads (fragile, deprecated libc API) | Tokio async tasks — one per player, update scheduler as interval |
| Error handling | Return codes, silently ignored | `Result<T, E>` — all errors must be handled or propagated |
| Persistence | Custom binary flat-file tables with hand-rolled cache | SQLite via `sqlx` — ACID, inspectable, migrations tracked |
| Build system | Autoconf/Automake (breaks on modern glibc) | Cargo — standard, reproducible |
| Dependencies | POSIX only, hand-ported Win32 shims | Rich ecosystem: `tokio`, `sqlx`, `serde`, `tracing`, `clap` |

---

## Codebase Layout

```
empire5/
├── README.md                  ← this file
├── Cargo.toml                 ← workspace manifest
├── config/
│   └── empire.toml            ← default server configuration
├── migrations/                ← top-level SQLite migration scripts
├── reference/
│   └── include/               ← original C headers (read-only reference)
└── crates/
    ├── empire-types/          ← core game-object structs and enums (no I/O)
    ├── empire-config/         ← TOML configuration loader
    ├── empire-db/             ← SQLite persistence layer (sqlx)
    ├── empire-server/         ← async TCP server binary (tokio)
    ├── empire-client/         ← terminal client binary
    ├── empire-world/          ← world generator (fairland port)
    └── empire-util/           ← CLI utilities: dump, sched, pconfig
```

---

## Architecture

### Threading model

Empire 4.x uses cooperative (non-preemptive) threads — each player session is a lightweight process that yields explicitly. Rust replaces this with **Tokio async tasks**:

- `tokio::net::TcpListener` accepts connections; each connection spawns a `tokio::task`.
- The update engine runs as a `tokio::time::interval` task holding an exclusive async `RwLock` while updating; player command tasks hold a shared lock.
- No manual stack management, no `makecontext`, no `SIGALRM`.

### Data layer

Empire 4.x stores all game state in hand-rolled binary flat files (`struct empfile`) with a seqno-stamped in-memory cache. Empire 5 uses **SQLite via `sqlx`**:

- One table per game object type (sectors, ships, planes, land units, nukes, nations, …).
- Schema migrations tracked in `crates/empire-db/migrations/`.
- `xdump` / `xundump` text format preserved for save/restore compatibility.

### Protocol

The original text-based TCP protocol (port 6665) is preserved exactly. The C client continues to work against the Rust server. Protocol evolution happens after the feature parity milestone.

---

## Modernization Roadmap

### Phase 0 — Foundation ✅ (current)
- [x] Cargo workspace with 7 crates
- [x] Core game-object types (`empire-types`)
- [x] TOML config system (`empire-config`)
- [x] SQLite schema + migrations (`empire-db`)
- [x] Tokio async server skeleton with command dispatch (`empire-server`)
- [x] Stubs for client, world generator, utilities

### Phase 1 — Data Layer ✅
- [x] Port `struct empfile` flat-file tables to SQLite
- [x] Port `xdump` / `xundump` (text dump/restore)
- [x] Port `ef_read`, `ef_write`, `ef_scan` iteration to type-safe SQL queries
- [x] Port `nsc.c` selector mini-language (parser + evaluator) to Rust

### Phase 2 — Server Core ✅
- [x] Port player connection/login flow (`src/lib/player/`)
- [x] Port I/O queue (`src/lib/gen/ioqueue.c`) to tokio buffered I/O
- [x] Port journal logging (`src/lib/common/journal.c`)
- [x] Port per-player state machine (PS_INIT → PS_PLAYING → PS_SHUTDOWN)
- [x] bcrypt password storage, SessionRegistry duplicate-login detection

### Phase 3 — Update Engine ✅
- [x] Port `server/update.c` and core of `src/lib/update/`
  - [x] Economic: populace accounting, tax, bank income, sector production
  - [x] Military: enlistment, mobility accrual for sectors/ships/planes/land units
  - [x] Nation levels: tech/research/education/happiness accumulation + aging
  - [x] Sector and product descriptor tables (dchr/pchr equivalent)
  - [x] Update rate constants in empire-config (all econfig-spec.h parameters)
- [x] Port market update (`server/marketup.c`) — `marketup.rs` Tokio task, 5-min cadence, gated by `opt_market`; `check_market`/`check_trade` bodies stub until buy/sell commands land (Phase 6+)
- [x] Port update scheduler schedule-file support — `rdsched.rs` in `empire-config` (port of `rdsched.c`); `run_update_loop` uses schedule file when present, falls back to `update_interval_secs`; `empsched` utility implemented

### Phase 4 — Game Subsystems (`src/lib/subs/`, 64 files) ✅
Priority order by coupling:
- [x] `geo.rs` — directions, neighbors, mapdist, coordinate formatting (ports `dir.c`, `xy.c`, `mapdist.c`)
- [x] `damage.rs` — damage application for ships/land/planes/sectors (ports `damage.c`)
- [x] `control.rs` — military control and sector abandonment (ports `control.c`)
- [x] `takeover.rs` — sector and unit takeover, CHE generation (ports `takeover.c`)
- [x] `nat_util.rs` — nation display and name validation (ports `natsub.c`, `natarg.c`)
- [x] `che`/`che_target` fields added to `Sector` (migration 003)
- [x] `get_at_xy` added to DB layer for land units, ships, planes
- [ ] `attsub.c` — attack resolution (2,589 lines, most complex) — Phase 5
- [ ] `lndsub.c`, `shpsub.c`, `plnsub.c` — unit management — Phase 5
- [ ] `mission.c` — standing orders — Phase 5
- [ ] `aircombat.c` — air combat — Phase 5
- [ ] `pathfind.c` — movement — Phase 5
- [ ] Remaining map, commodity, trade subsystems — Phase 5

### Phase 5 — Commands (`src/lib/commands/`, 151 files) ✅
Each command is an `async fn` in `empire-server/src/commands/`, dispatched via `CmdCtx`.

Core commands implemented:
- [x] `census` / `cens` — sector-by-sector report with distribution and threshold display
- [x] `nation` / `nati` — nation status, capital, treasury, education, tech, research
- [x] `map` / `bmap` / `smap` / `sect` / `sector` — toroidal hex world map with border
- [x] `designate` / `desi` — redesignate sector type (validates coastal, deity-only types)
- [x] `threshold` / `thre` — set/display commodity distribution thresholds per sector
- [x] `relations` / `rela` — display diplomatic relations (yours vs theirs)
- [x] `declare` / `decl` — declare diplomatic stance toward other nations
- [x] `version` / `vers` — server version, world dimensions, ETU
- [x] `info` — topic help text
- [x] `xdump` — structured data dump (nations, sectors, relations)
- [x] Migration 004: `thresholds_json` column on sectors; `relations` table
- [x] `CmdCtx` — per-command context struct (cnum, nation, db, world dims, ETU)

Remaining (Phase 6+):
- [ ] `attsub.c` — attack resolution (2,589 lines, most complex)
- [ ] `lndsub.c`, `shpsub.c`, `plnsub.c` — unit management
- [ ] `mission.c` — standing orders
- [ ] `aircombat.c` — air combat
- [ ] `pathfind.c` — movement
- [ ] Economic: `build`, `distribute`, `deliver`, `produce`, `work`
- [ ] Military: `attack`, `march`, `navigate`, `bomb`, `fly`, `launch`
- [ ] Trade/finance: `buy`, `sell`, `trade`, `loan`
- [ ] Admin/deity: `edit`, `enable`, `add`, `new`, `wipe`, `shutdown`
- [ ] Info: `show`, `power`, `news`, `dump`

### Phase 6 — World Generator & Utilities ✅
- [x] `empire-world` binary — full port of `fairland.c` (1,681 lines) + `files.c`
  - Capital drift (perturbation technique) to maximise inter-capital distance
  - Weighted random continent & island growing with spike control
  - Elevation creation (random walk + plateau/mountain classification)
  - Resource computation (iron, gold, fertility, oil, uranium) from elevation curves
  - Sector DB write via `sectors::put_many` (all 1,024 valid hex positions for a 64×32 world)
  - `newcap_script` generation for deity setup
  - Deity nation ("POGO") and visitor slot auto-created on first run
  - CLI: `empire-world [OPTIONS] NC SC [NI [IS [SP [PM [DI [ID]]]]]]`
- [ ] `empdump` — export/import via xdump (deferred; server xdump already works)
- [x] `empsched` — print update schedule (reads schedule file, prints next N times)
- [ ] `pconfig` — print config values (deferred; trivial)

### Phase 7 — Client (optional)
- Keep C client as-is (compatible with Rust server protocol)
- Or: Rust client using `tokio` + `rustyline`
- Long-term option: WebSocket proxy + browser client

---

## Feature Parity Checklist

The following game systems must all pass the reference server comparison suite before 5.0 is declared stable:

- [ ] Nation creation, login, sanctuary, active status
- [ ] Sector map, designation changes, efficiency
- [ ] Commodity storage and movement (distribute, deliver)
- [ ] Civilian and military production cycles
- [ ] Land unit: build, march, attack, retreat
- [ ] Naval unit: build, navigate, board, torpedo, fire
- [ ] Air unit: build, fly, bomb, recon, paradrop, satellite
- [ ] Nuclear weapons: build, arm, launch, detonate, fallout
- [ ] Combat: land assault, naval combat, air combat, missions
- [ ] Trade: buy, sell, market, loans
- [ ] Diplomacy: declarations, telegrams, news
- [ ] Update engine: ETU tick, mobility, populace, production
- [ ] Deity admin: edit, enable, add nation, wipe
- [ ] xdump / xundump round-trip fidelity

---

## Building

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the entire workspace
cargo build

# Run the server (development mode)
cargo run -p empire-server -- --config config/empire.toml

# Run tests
cargo test --workspace

# Build optimized release
cargo build --release
```

## Running Alongside Empire 4.4.1

The C reference server runs on port 6665. Run the Rust server on a different port during development:

```bash
# Reference server (C)
cd ../empire4.4.1 && ./src/server/empire -e econfig

# Development server (Rust) on alternate port
cargo run -p empire-server -- --config config/empire.toml --port 6666
```

Use the C client against both servers to compare behavior:

```bash
# Against reference
empire -s localhost -p 6665

# Against Rust dev server
empire -s localhost -p 6666
```

---

## License

Empire 5 is distributed under the GNU General Public License v3.
See [COPYING](COPYING) for the full license text and [CREDITS](CREDITS) for the
full list of contributors.

Original Empire authors (1986–2021): Dave Pare, Jeff Bailey, Thomas Ruschak,
Ken Stevens, Steve McClure, Markus Armbruster, and many other contributors.

Empire 5 Rust rewrite: Dave Nye, with AI assistance from Claude (Anthropic).
