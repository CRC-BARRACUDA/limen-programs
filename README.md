# programs

A native [Limen](https://github.com/CRC-BARRACUDA/Limen) module that lists the
software **installed on this machine** — the local equivalent of Add/Remove
Programs, extended to catch user-local installs.

Provides the `programs.local` capability.

## Sources

**Windows** — the `Uninstall` registry keys behind Add/Remove Programs:

- `HKLM\…\CurrentVersion\Uninstall` (64-bit machine-wide)
- `HKLM\…\WOW6432Node\…\Uninstall` (32-bit machine-wide)
- `HKCU\…\CurrentVersion\Uninstall` (**per-user** installs)

OS components (`SystemComponent`) and updates/patches are filtered out.

**Linux** — several sources, so both system and **user-local** installs show up:

- `dpkg` / `rpm` — system package managers
- `flatpak` / `snap` — universal packages (user or system)
- XDG desktop entries in `/usr/share/applications`,
  `/usr/local/share/applications`, and **`~/.local/share/applications`**
- **`~/.local/bin`** — user-installed executables (pip/pipx, cargo, manual)
- `/opt` — vendor install trees

Every entry is reported in one schema: `source`, `name`, `version`,
`publisher`, `location`, `scope` (`system`/`user`).

## Using it

Open the **Programs** view and press **Scan** — it does not enumerate on open.
The results table is interactive: right-click a row for **About** or **Open
location**, or double-click for its details. If a `report.build` provider is
loaded, a **Make Report** action appears (charts + tables, exportable to
Markdown / HTML / CSV).

Other modules can call `programs.local` → `list` for the raw inventory.

## Building

Native (`cdylib`) module built with `limen-sdk-rust`. For local testing before
the SDK is published, point the git SDK dependency at a local checkout:

```sh
LIMEN_SDK_PATH=/path/to/Limen/src/limen-sdk-rust scripts/package.sh
```

This produces `dist/programs-<os>-<arch>.<ext>` and its `.sha256`, and refreshes
the in-repo loadable so a locally-run Limen picks it up on **Reload**.

## License

GPL-3.0-or-later.
