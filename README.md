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

## Current Status: Feature-Complete Beta

All major game systems are implemented. The server is running live on **TekBot** (192.168.40.95:6665) with an active 6-player game world. The original Empire C client connects and plays against the Rust server without modification.

### What's Working Right Now

| System | Status |
|---|---|
| World generation (fairland) | ✅ Full port — correct hex grid, toroidal wrap, land/island/mountain placement |
| Player login (TCP protocol) | ✅ Full protocol — user/coun/pass/play pre-login flow, bcrypt passwords |
| Session management | ✅ Duplicate-login detection, kill command, journal logging |
| Update engine (ETU ticks) | ✅ Full port — populace, production, mobility, nation levels, schedule files |
| Map display | ✅ Fog-of-war per-nation bmap; `map` shows last-seen terrain, own sectors always current |
| `census` | ✅ Full sector report with efficiency, civilians, commodities, coastal flag |
| `map` / `bmap` / `smap` | ✅ Toroidal hex world map with configurable realm, fog of war for non-deity |
| `radar` | ✅ Sweep `)` sectors to reveal terrain; range formula matches 4.4.1 `techfact()` |
| `nation` | ✅ Nation status, treasury, tech, research, education, happiness |
| `designate` | ✅ Sector redesignation with type validation |
| `explore` | ✅ Move civilians into wilderness to claim territory |
| `move` | ✅ Move commodities between owned sectors |
| `distribute` / `deliver` | ✅ Distribution centers and commodity delivery thresholds |
| `threshold` | ✅ Per-sector per-commodity storage thresholds |
| `production` | ✅ Simulate next-update production output for any sector spec |
| `relations` / `declare` | ✅ Diplomacy — stance declarations (neutral/hostile/allied) |
| `build s\|l\|p` | ✅ Build ships, land units, planes from sectors |
| `build b\|t` | ✅ Build bridge spans (`=`) and towers (`@`) from bridge heads (`#`) |
| `march` | ✅ Land unit movement (direction string or X,Y destination) |
| `navigate` | ✅ Ship navigation (direction string or X,Y destination) |
| `attack` | ✅ Ground combat with att/def strength, tech bonus, takeover on win |
| `bomb` / `fly` / `launch` / `mission` | ✅ Air combat — bomb, relocate planes, fire missiles, standing orders |
| `fire` / `torpedo` | ✅ Ship/land/fort gunnery and submarine torpedo combat, no-miss shelling with return fire |
| `satellite` / `launch` (orbital) / `lradar` | ✅ Launch and query orbital recon satellites; long-range radar via satellite |
| `recon` / `sweep` | ✅ Multi-sector reconnaissance flight (SPY-vs-generic report per hex); sweep additionally clears naval mines |
| `fleetadd` / `army` | ✅ Group ships/land units by letter, addressable from navigate/fire/torpedo/tend/march/attack |
| `load` / `unload` (plane) | ✅ Put planes aboard (or off) a carrier or missile sub; `fly`/`recon`/`sweep` also land directly on a friendly carrier at the destination |
| `sell` / `buy` / `trade` / `loan` | ✅ Commodity market and P2P lending |
| `show` | ✅ Build cost/stat tables for sectors, ships, land units, planes, items |
| `power` | ✅ Nation power rankings |
| `news` | ✅ In-game news feed of recent world events |
| `telegram` / `read` / `announce` | ✅ Player-to-player messages and broadcast announcements |
| `add` / `capital` / `newcap` | ✅ Deity nation management |
| `enable` / `disable` / `shutdown` | ✅ Deity server control |
| `info` | ✅ 76 help pages covering every implemented command |
| `xdump` | ✅ Structured data export (nations, sectors, relations, ships, planes, units) |
| `version` | ✅ Server version, world dimensions, ETU |
| Docker deployment | ✅ Multi-stage Dockerfile, named volume, container running on TekBot |
| SQLite durability | ✅ WAL mode + `synchronous=Full` — survives container kill |

---

## Codebase Layout

```
empire5/
├── README.md                  ← this file
├── Cargo.toml                 ← workspace manifest
├── Dockerfile                 ← multi-stage build (rust:1.87-slim → debian:bookworm-slim)
├── docker-compose.yml         ← TekBot deployment (container: tekbot, port 6665)
├── config/
│   └── empire.toml            ← default server configuration
├── docker/
│   ├── empire.toml            ← TekBot production config (96×64 world)
│   └── entrypoint.sh          ← container startup script
├── info/                      ← 76 official Empire help pages
├── migrations/                ← top-level SQLite migration scripts
├── reference/
│   └── include/               ← original C headers (read-only reference)
└── crates/
    ├── empire-types/          ← core game-object structs and enums (no I/O)
    ├── empire-config/         ← TOML configuration loader
    ├── empire-db/             ← SQLite persistence layer (sqlx)
    ├── empire-server/         ← async TCP server binary (tokio)
    ├── empire-client/         ← terminal client binary (Phase 11 stub)
    ├── empire-world/          ← world generator (fairland port)
    └── empire-util/           ← CLI utilities: empdump, empsched, pconfig
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
- WAL journal mode with `synchronous=Full` for crash durability.
- `xdump` / `xundump` text format preserved for save/restore compatibility.

### Protocol

The original text-based TCP protocol (port 6665) is preserved exactly. The C client continues to work against the Rust server without modification. Response codes: `C_DATA(1)`, `C_INIT(2)`, `C_CMDOK(5)`, `C_PROMPT(6)`, `C_CMDERR(10)`, `C_BADCMD(11)`, `C_EXIT(14)`.

### Hex grid

Valid sectors satisfy `(x + y) % 2 == 0` on a toroidal world. Direction offsets match Empire 4's `dir.c` exactly:
- UR=(1,-1), R=(2,0), DR=(1,1), DL=(-1,1), L=(-2,0), UL=(-1,-1)
- Direction chars: `u`=UR, `j`=R, `n`=DR, `b`=DL, `g`=L, `y`=UL, `h`=stop

---

## Modernization Roadmap

### Phase 0 — Foundation ✅
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
- [x] Port I/O queue to tokio buffered I/O
- [x] Port journal logging
- [x] Port per-player state machine (PS_INIT → PS_PLAYING → PS_SHUTDOWN)
- [x] bcrypt password storage, SessionRegistry duplicate-login detection

### Phase 3 — Update Engine ✅
- [x] Port `server/update.c` and core of `src/lib/update/`
  - [x] Economic: populace accounting, tax, bank income, sector production
  - [x] Military: enlistment, mobility accrual for sectors/ships/planes/land units
  - [x] Nation levels: tech/research/education/happiness accumulation + aging
  - [x] Sector and product descriptor tables (dchr/pchr equivalent)
  - [x] Update rate constants in empire-config (all econfig-spec.h parameters)
- [x] Port market update task (`marketup.rs`) — 5-min cadence, gated by `opt_market`
- [x] Port update scheduler schedule-file support (`rdsched.rs`)

### Phase 4 — Game Subsystems ✅
- [x] `geo.rs` — directions, neighbors, mapdist, coordinate formatting
- [x] `damage.rs` — damage application for ships/land/planes/sectors
- [x] `control.rs` — military control and sector abandonment
- [x] `takeover.rs` — sector and unit takeover, CHE generation
- [x] `nat_util.rs` — nation display and name validation
- [x] `shpsub.rs` — ship fire/damage/range helpers
- [x] `lndsub.rs` — land unit fire/damage/support helpers
- [x] `plnsub.rs` — plane capability/damage/fuel helpers
- [x] `aircombat.rs` — 3-round air-vs-air combat resolution; async interceptor scan
- [x] `attsub.rs` — ground combat: att_str/def_str with tech + fort bonus
- [x] `pathfind.rs` — BFS pathfinding on toroidal hex grid

### Phase 5 — Commands ✅
Core commands (port of `src/lib/commands/`, 151 C files):
- [x] `census` — sector report: efficiency, civilians, commodities, coastal flag, thresholds
- [x] `nation` — nation status, treasury, tech, research, education, happiness
- [x] `map` / `bmap` / `smap` / `sect` — toroidal hex map with fog of war (per-nation bmap)
- [x] `radar` — sweep `)` radar sectors to reveal terrain; tech-scaled range
- [x] `designate` — sector redesignation with type validation
- [x] `threshold` — per-sector per-commodity storage thresholds
- [x] `relations` / `declare` — diplomacy: view and set stance toward other nations
- [x] `distribute` / `deliver` — distribution centers and delivery thresholds
- [x] `production` / `prod` — simulate next-update production output
- [x] `news` — in-game event feed
- [x] `version` / `info` / `xdump` — server metadata and data export
- [x] `show sect/ship/land/plane/item/product/updates` — all descriptor tables
- [x] `power` — nation power rankings
- [x] `add` / `capital` / `newcap` / `enable` / `disable` / `shutdown` — deity commands
- [x] `build s|l|p` — build ships, land units, planes
- [x] `build b|t` — build bridge spans (`=`) and bridge towers (`@`)
- [x] `march` — land unit movement
- [x] `navigate` — ship navigation
- [x] `attack` — ground combat
- [x] `bomb` / `fly` / `launch` / `mission` — air operations
- [x] `fire` / `torpedo` — ship/land/fort gunnery, submarine torpedoes
- [x] `satellite` / `lradar` — orbital reconnaissance, long-range radar
- [x] `recon` / `sweep` — multi-sector reconnaissance flight, minesweeping
- [x] `fleetadd` / `army` — group ships/land units for letter-based targeting
- [x] `load` / `unload` (plane) — put planes aboard carriers/missile subs
- [x] `sell` / `buy` / `trade` / `loan` — market and finance
- [x] `explore` — move civilians into adjacent wilderness to claim territory
- [x] `move` — move commodities between owned sectors
- [x] `telegram` / `read` / `announce` — player messaging and broadcasts

### Phase 6 — World Generator & Utilities ✅
- [x] `empire-world` — full port of `fairland.c` (1,681 lines) + `files.c`
  - Capital drift to maximise inter-capital distance
  - Weighted random continent & island growing with spike control
  - Elevation creation (random walk + plateau/mountain classification)
  - Resource computation (iron, gold, fertility, oil, uranium) from elevation curves
  - Correct hex DIROFF matching Empire 4 `dir.c` (critical bug fix)
  - `newcap_script` generation for deity setup
  - CLI: `empire-world [OPTIONS] NC SC [NI [IS [SP [PM [DI [ID]]]]]]`
- [x] `empdump` — standalone xdump export (all tables or named tables)
- [x] `empsched` — print update schedule
- [x] `pconfig` — print effective config values

### Phase 7 — Docker Deployment ✅
- [x] Multi-stage `Dockerfile` (rust:1.87-slim builder → debian:bookworm-slim runtime)
- [x] `docker-compose.yml` with named volume `tekbot-empire-data` at `/srv/empire/data`
- [x] `docker/empire.toml` — production config: 96×64 world, 6 nations, 60 ETU
- [x] `docker/entrypoint.sh` — auto-runs world gen on first start, then starts server
- [x] Live deployment on TekBot (192.168.40.95:6665), 6 active player slots
- [x] 79 official Wolfpack Empire info pages installed in `info/`

### Phase 8 — Open Items
- [ ] `resource` / `report` / `commodity` commands (stubs in dispatch; output incomplete)
- [ ] `edit` — deity sector/nation/unit editing
- [ ] `empire-client` Rust client (Phase 11 placeholder; use C client from empire4.4.1)

---

## Feature Parity Checklist

- [x] Nation creation, login, sanctuary, active status
- [x] Sector map, designation changes, efficiency
- [x] Commodity storage and movement (explore, move, distribute, deliver)
- [x] Civilian and military production cycles (update engine)
- [x] Land unit: build, march, attack
- [x] Naval unit: build, navigate
- [x] Air unit: build, fly, bomb, launch, mission
- [x] Combat: land assault, air combat
- [x] Trade: buy, sell, market, loans
- [x] Diplomacy: declarations, relations
- [x] Update engine: ETU tick, mobility, populace, production, nation levels
- [x] Deity admin: add nation, newcap, enable/disable, shutdown
- [x] xdump / xundump round-trip fidelity
- [x] Docker containerized deployment
- [x] Fog of war — per-nation bmap updated by radar sweeps and own-sector visibility
- [x] Bridge building — spans (`=`) and towers (`@`) with tech requirements
- [x] Radar command — sweep `)` sectors, tech-scaled range
- [x] Production simulation — `prod` command
- [x] News feed — `news` command with correct schema migration
- [x] Naval/land/fort gunnery and torpedo combat (`fire`, `torpedo`)
- [x] Orbital reconnaissance and long-range radar (`satellite`, `lradar`)
- [x] Fleet/army grouping for letter-based unit targeting (`fleetadd`, `army`)
- [x] Multi-sector recon flights and naval minesweeping (`recon`, `sweep`)
- [x] Carrier and missile-sub plane loading (`load`/`unload` plane, carrier landing on `fly`/`recon`/`sweep`)
- [x] Telegrams / player messaging (`telegram`, `read`, `announce`)
- [ ] Nuclear weapons: build, arm, launch, detonate, fallout
- [ ] Standing missions firing during update
- [ ] `edit` command (deity sector/nation/unit editing)

---

## Building

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the entire workspace
cargo build

# Build optimized release
cargo build --release

# Run tests
cargo test --workspace

# Run the server (development mode)
cargo run -p empire-server -- --config config/empire.toml
```

---

## Docker Deployment (Recommended)

### Quick start

```bash
# Build image and start server
docker-compose up -d

# Watch logs
docker-compose logs -f

# Connect with C client
../empire4.4.1/src/client/empire -s localhost:6665 1 1
```

### Build and deploy to a remote host

```bash
# Build image locally
docker build -t tekbot-empire:latest .

# Push to remote host (e.g. TekBot at 192.168.40.95)
docker save tekbot-empire:latest | ssh user@192.168.40.95 "docker load"

# On the remote host, start the container
ssh user@192.168.40.95 "docker stop tekbot; docker start tekbot"
```

### Data volume

All game state lives in the named Docker volume `tekbot-empire-data` mounted at `/srv/empire/data/`:
- `empire.db` — SQLite game database (WAL mode, `synchronous=Full`)
- `empire.db-shm` / `empire.db-wal` — WAL files (always copy all three together)
- `info/` — help pages
- `journal` — server activity log
- `newcap_script` — deity setup commands generated by world gen

---

## Starting a Fresh Game World (Manual / Non-Docker)

### 1. Prerequisites

| Tool | How to install |
|---|---|
| Rust toolchain | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| C client (empire) | Build from `../empire4.4.1/` — `./bootstrap && ./configure && make` |

### 2. Build Empire 5

```bash
cd empire5
cargo build --release
# Binaries: target/release/{empire-server,empire-world,empdump,empsched,pconfig}
```

### 3. Create Game Directory

```bash
mkdir -p /srv/empire/mygame
cd /srv/empire/mygame
cp /path/to/empire5/config/empire.toml .
```

Edit `empire.toml`:

```toml
[server]
port = 6665
data_dir = "data"
info_dir = "/path/to/empire5/info"

[game]
world_x = 96
world_y = 64
etu_per_update = 60
```

### 4. Generate the World

```bash
# empire-world NC SC [NI [IS [SP [PM]]]]
#   NC = player nations, SC = continent size, NI = islands,
#   IS = island size, SP = spike %, PM = mountain %
./empire-world -e empire.toml 6 40 12 20 10 5
```

### 5. Deity Setup

Connect as POGO (country 0, blank password) and run `newcap_script`:

```bash
./empire -s localhost:6665
# Country name: POGO
# Password: (blank)

# Paste newcap_script contents, then:
enable
```

### 6. Players Connect

```bash
# Format: empire -s host:port country_name password
./empire -s 192.168.40.95:6665 1 1    # country 1, password 1
```

**Note:** When the C client shows `Your name?` it is actually prompting for the **password** (mislabeled in Empire 4.4.1's `login.c`). If your terminal shows `^M` on Enter (stuck in raw mode from a crashed `getpass`), run `reset` to fix it.

### Quick-Reference: Key Commands

| Command | What it does |
|---|---|
| `census *` | Show all your sectors |
| `map 0,0` | Display map around capital |
| `nation` | Treasury, tech, research, happiness |
| `explore X,Y PATH civ N` | Move civilians to claim wilderness |
| `move X,Y PATH ITEM N` | Move commodities between sectors |
| `designate X,Y TYPE` | Change sector type (`a`=agri, `h`=harbor, `t`=tech…) |
| `show sect b` | Sector build-cost and production table |
| `build s X,Y TYPE` | Build a ship at a harbor |
| `march UNITS PATH` | Move land units |
| `attack X,Y` | Attack an enemy sector |
| `power` | Nation power rankings |
| `add N NAME REP p` | Deity: create player nation |
| `enable` / `disable` | Deity: start/stop update engine |

### Sector Type Mnemonics

| Char | Type | Char | Type |
|---|---|---|---|
| `.` | sea (ocean) | `-` | wilderness (unexplored land) |
| `^` | mountain | `c` | capital (urban) |
| `a` | agribusiness | `h` | harbor |
| `t` | tech center | `r` | research lab |
| `b` | bank | `l` | light industry |
| `k` | heavy industry | `e` | engineer |
| `d` | defense plant | `f` | fortress |
| `*` | airfield | `+` | highway |
| `g` | gold mine | `o` | oil field |
| `%` | wasteland | `n` | naval base |
| `#` | bridge head | `=` | bridge span |
| `@` | bridge tower | `)` | radar |

---

## License

Empire 5 is distributed under the GNU General Public License v3.
See [COPYING](COPYING) for the full license text and [CREDITS](CREDITS) for the full list of contributors.

Original Empire authors (1986–2021): Dave Pare, Jeff Bailey, Thomas Ruschak, Ken Stevens, Steve McClure, Markus Armbruster, and many other contributors.

Empire 5 Rust rewrite: Dave Nye, with AI assistance from Claude (Anthropic).
