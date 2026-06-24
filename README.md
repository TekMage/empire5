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

### Phase 1 — Data Layer
- Port `struct empfile` flat-file tables to SQLite
- Port `xdump` / `xundump` (text dump/restore)
- Port `ef_read`, `ef_write`, `ef_scan` iteration to type-safe SQL queries
- Port `nsc.c` selector mini-language (parser + evaluator) to Rust

### Phase 2 — Server Core
- Port player connection/login flow (`src/lib/player/`)
- Port I/O queue (`src/lib/gen/ioqueue.c`) to tokio buffered I/O
- Port journal logging (`src/lib/common/journal.c`)
- Port per-player state machine (PS_INIT → PS_PLAYING → PS_SHUTDOWN)

### Phase 3 — Update Engine
- Port `server/update.c` and all of `src/lib/update/`
  - Economic: populace, mobility, delivery, production
  - Military: sector repair, unit recovery
- Port market update (`server/marketup.c`)
- Port update scheduler (`src/util/empsched.c`, `src/lib/common/rdsched.c`)

### Phase 4 — Game Subsystems (`src/lib/subs/`, 64 files)
Priority order by coupling:
1. `attsub.c` — attack resolution (2,589 lines, most complex)
2. `lndsub.c`, `shpsub.c`, `plnsub.c` — unit management
3. `mission.c` — standing orders
4. `aircombat.c` — air combat
5. `pathfind.c` — movement (already in `lib/common/`)
6. Remaining map, commodity, trade subsystems

### Phase 5 — Commands (`src/lib/commands/`, 151 files)
Each command becomes an `async fn` in `empire-server/src/commands/`. Assign by group:

| Group | Commands | Est. weeks |
|---|---|---|
| Navigation/mapping | `map`, `look`, `census`, `sector`, `bestpath` | 2 |
| Economic | `build`, `distribute`, `deliver`, `produce`, `work` | 3 |
| Military — land | `attack`, `march`, `fortify`, `arm` | 3 |
| Military — naval | `navigate`, `torpedo`, `fire`, `board` | 2 |
| Military — air | `bomb`, `fly`, `launch`, `paradrop`, `recon` | 3 |
| Trade/finance | `buy`, `sell`, `trade`, `loan`, `shark` | 2 |
| Diplomacy | `declare`, `relations`, `reject`, `telegram` | 1 |
| Admin/deity | `edit`, `enable`, `add`, `new`, `wipe`, `shutdown` | 2 |
| Info/dump | `show`, `nation`, `power`, `news`, `dump`, `xdump` | 2 |

### Phase 6 — World Generator & Utilities
- Port `fairland.c` (world generator, 1,681 lines)
- Port `empdump`, `empsched`, `pconfig`, `files`

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
