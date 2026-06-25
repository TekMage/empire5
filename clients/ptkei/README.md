# PTkEI — Python/Tk Empire Interface

**Version 3.0.0** — Python 3 port for Empire 5 / Empire 4.4.x protocol

PTkEI is a graphical Empire client built with Python and Tkinter. It provides
a full GUI with a sector map, census window, telegram browser, bestpath
overlay, and command history. This version is a Python 3 port of the original
PTkEI 2.00.0, updated to work with the Empire 5 Rust server and the
Empire 4.4.1 wire protocol.

---

## Quick Start

```bash
# Install dependencies (macOS with Homebrew)
brew install python@3.14 python-tk@3.14
pip3 install --break-system-packages Pmw

# Run (connects to server configured in empDb.py)
python3 empire.py
```

Default server: `192.168.40.95:6665` — edit `src/empDb.py` to change.

---

## What Changed in 3.0.0 (Python 3 Port)

### Python 2 → Python 3 migration (all 19 source files)

| Change | Detail |
|--------|--------|
| `import Tkinter` | → `import tkinter as Tkinter` |
| `import tkMessageBox` | → `from tkinter import messagebox as tkMessageBox` |
| `import tkFileDialog` | → `from tkinter import filedialog as tkFileDialog` |
| `import cPickle` | → `import pickle as cPickle` |
| `print stmt` | → `print()` function |
| `raw_input()` | → `input()` |
| `has_key()` | → `in` operator |
| `.iteritems()` | → `.items()` |
| `.itervalues()` | → `.values()` |
| `raise X, msg` | → `raise X(msg)` |
| `except X, e` | → `except X as e` |
| `` `expr` `` | → `repr(expr)` |
| `apply(f, args, kw)` | → `f(*args, **kw)` |
| `string.join()` | → `str.join()` |
| `xrange()` | → `range()` |
| `basestring` | → `str` |
| `sys.exitfunc` | → `atexit.register()` |
| `__nonzero__` | → `__bool__` |
| `__div__` / `__rdiv__` | → `__truediv__` / `__rtruediv__` |
| Bundled Pmw (Python 2) | → pip Pmw 2.1.1 (Python 3 compatible) |

### Protocol fixes for Empire 4.4.x

| Change | Detail |
|--------|--------|
| `C_CMDERR = "a"` | → `"10"` (decimal code, not hex) |
| `C_BADCMD = "b"` | → `"11"` |
| Protocol parsing | Updated from single-char `line[:1]` to space-split for full decimal codes |
| Socket I/O | `recv().decode('latin-1')`, `send(str.encode('latin-1'))` |
| `async` variable | Renamed to `async_` (Python 3 reserved keyword) |

### Bug fixes

- **`Tk_VDB.getOption`**: Tk's option database `\ ` (backslash-space) empty sentinel is
  returned as `" "` (space) by Python 3's `option_get()`. Added whitespace-stripping
  so it no longer gets passed as a color value to `create_polygon`.
- **`empTk.InitFileHandler`**: `Tkinter.tkinter.createfilehandler` →
  `tkinter.createfilehandler` (Python 3 module structure).
- **`empEval` exec strings**: Previous auto-converters could corrupt `r"\n"` raw strings
  in exec calls. Restored correct non-raw `"\n"` strings so exec'd function
  definitions receive actual newlines.

### Defaults updated
- Default server host: `192.168.40.95`, port: `6665` (Empire 5 test server)

---

## Original Credits

See [doc/CREDITS](doc/CREDITS) and [doc/CREDITS2](doc/CREDITS2) for the full
original credit history.

**Original author:** Kevin O'Connor (1998–2000)  
**Continued by:** Laurent Martin (2001–2002)  
**2.x update:** William Fittge (2013) — restored compatibility with Empire 4.3.x  
**Testing / early beta:** Bernhard Reiter  
**Multi-move tool / bestpath:** Ulf Larsson  
**Uses:** PMW (Python Mega Widgets) — https://pmw.sourceforge.net

---

## Version 3.0.0 Credits

Python 3 port and Empire 4.4.x protocol fixes by **TekMage** (2026), with
AI-assisted conversion by Claude (Anthropic).

---

## License

GNU General Public License v2 — see [COPYING](COPYING).

Original copyright © 1998–2013 Kevin O'Connor, Laurent Martin, William Fittge.  
Version 3.0.0 modifications © 2026 TekMage.

---

## Requirements

- Python 3.9+
- `python-tk` (Tkinter support — separate package on some distros/Homebrew)
- `Pmw` 2.1.1+ (`pip install Pmw`)

## Tested Against

- Empire 5 Rust server (https://github.com/TekMage/empire5)
- Empire 4.4.1 wire protocol
- Python 3.14 on macOS (Homebrew)
