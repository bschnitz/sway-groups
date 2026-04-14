# Plan: Konfigurierbares swayg

## Motivation

Viele Aspekte von swayg sind aktuell hardcoded (Socket-Pfade, default Group/Workspace, Bar-Verhalten). Ziel ist es, alles über eine TOML-Config-Datei konfigurierbar zu machen, ohne dass eine Config zwingend nötig ist (alles hat sinnvolle Defaults).

## Neuer Crate: `sway-groups-config`

Ein eigener minimaler Crate im Workspace, der von CLI, Daemon und Core genutzt wird.

## Config-Struktur (TOML)

```toml
# ~/.config/swayg/config.toml

[defaults]
# Gruppe, die als Fallback für orphaned workspaces dient (z.B. bei repair/delete)
default_group = "0"
# Workspace, der als Fallback bei leeren Gruppen genutzt wird
default_workspace = "0"

[bar.workspaces]
# Socket-Instanzname für waybar-dynamic (wird zu $XDG_RUNTIME_DIR/waybar-dynamic-<name>.sock)
socket_instance = "swayg_workspaces"
# Was wird angezeigt: "all" (alle workspaces), "current" (nur aktive Gruppe), "none" (keine)
display = "all"
# Globale Workspaces in workspace bar zeigen oder ausblenden
show_global = true

[bar.groups]
socket_instance = "swayg_groups"
# Was wird angezeigt: "all" (alle Gruppen), "active" (nur aktive), "none" (keine)
display = "all"
# Leere Gruppen in groups bar zeigen
show_empty = true
```

## Config-Pfad

`directories` crate (bereits vorhanden):
- **Linux**: `~/.config/swayg/config.toml`
- Berechnet via: `ProjectDirs::from("com", "swayg", "swayg").config_dir() / "config.toml"`

## Neue CLI-Befehle

```
swayg config dump [-o|--output <path>]   # Default-Config in Datei schreiben (stdout wenn kein -o)
```

## Änderungen pro Crate

### `sway-groups-config` (NEU)

- `SwaygConfig` struct mit serde Deserialize + Serialize
- `SwaygConfig::load()` → liest Config von Standardpfad, `Ok(Self::default())` wenn Datei nicht existiert
- `SwaygConfig::load_from(path)` → liest Config von beliebigem Pfad
- `SwaygConfig::default()` → alle hardcoded defaults
- `SwaygConfig::dump_to(path)` / `SwaygConfig::dump()` → schreibt TOML
- `SwaygConfig::config_path()` → berechnet Standardpfad
- Abhängigkeiten: `serde`, `toml`, `directories`

### `sway-groups-core`

- `WaybarSyncService::new()` nimmt `&SwaygConfig` (oder Teile davon)
- `WaybarClient` bekommt Option für custom socket instance name
- `group_service.rs`: `"0"` → `config.default_group` (in `delete_group`, `repair`)
- `group_service.rs`: Fallback workspace `"0"` → `config.default_workspace` (in `set_active_group`)
- `visibility_service.rs`: `"0"` catch-all → `config.default_group`

### `sway-groups-cli`

- Globaler Flag: `--config <path>` (optional, überschreibt Standardpfad)
- `commands.rs`: Config laden, an Services weitergeben
- `swayg config dump` Subcommand
- `swayg init`: Config wird nicht in DB gespeichert — Daemon und CLI lesen independently
- `swayg sync --init-bars`: Liest Config für socket/display Einstellungen

### `sway-groups-daemon`

- Liest Config beim Start (gleicher Standardpfad wie CLI)
- Nutzt Config für socket paths, default group (bei workspace assignment)

## Display-Modi

| Wert | Bar Groups | Bar Workspaces | Socket-Kommunikation |
|------|-----------|----------------|---------------------|
| `"all"` | Alle Gruppen | Alle (gefiltert nach aktiver Gruppe) | Ja |
| `"active"` | Nur aktive Gruppe | — | Ja |
| `"none"` | — | — | **Nein (no-op)** |

- `"none"` → `update_waybar()` / `update_waybar_groups()` werden no-ops (return Ok sofort)
- `swayg sync --init-bars` mit `display = "none"` → skippt den jeweiligen init-bars Schritt

## Init-Bars Reload

`swayg sync --init-bars` liest Config neu und:
1. Prüft `bar.workspaces.display` → wenn nicht `"none"`: sendet widgets an workspace socket
2. Prüft `bar.groups.display` → wenn nicht `"none"`: sendet widgets an groups socket

## Tests

- `swayg config dump` → stdout enthält default Config
- `swayg config dump -o /tmp/test.toml` → Datei enthält default Config
- Bestehende Tests nutzen default Config (keine Config-Datei nötig)
- Tests können Config via `--config` Flag mit custom Pfad nutzen

## Phasen

1. **Config Crate**: `sway-groups-config` erstellen mit `SwaygConfig`, load/dump/default
2. **Core Integration**: Services nehmen Config, `"0"` hardcodes → `config.default_group`
3. **CLI Integration**: `--config` Flag, `swayg config dump`, Config laden + weitergeben
4. **Daemon Integration**: Config beim Start laden
5. **Bar display modes**: `display = "none"` → no-ops, `"all"` / `"active"` Logik
6. **Init-bars Reload**: `--init-bars` respektiert Config
7. **Tests**: Config-Tests, bestehende Tests mit default Config laufen lassen
8. **Dokumentation**: AI_TEST_INSTRUCTIONS.md updaten
