# Refactor Plan: CLI-only Architecture

## Problem

`swayg` CLI und `swaygd` Daemon greifen gleichzeitig auf dieselben Ressourcen zu:
- Beide schreiben in die DB (`outputs.active_group`, `group_state`)
- Beide senden `rename workspace` via sway IPC
- Beide lesen Workspaces von sway und berechnen Suffixe

Keine Koordination -> Race Conditions bei Gruppenwechseln, falsche Suffixe, falscher Workspace-Fokus.

Der Daemon-Lösungsansatz (Queue/Socket) führte zu einem Feedback-Loop: `sync_all_suffixes` renamt Workspaces -> Sway Events -> Daemon reagiert auf Events -> neuer sync -> neuer Rename -> ...

## Zielarchitektur

**Kein Daemon.** Die CLI ist der einzige Aktor.

Jeder `swayg`-Aufruf:
1. Liest DB und Sway-Status
2. Führt Mutationen durch (DB + Sway IPC)
3. Führt `sync_all_suffixes()` durch

Neue Workspaces (von externen Tools erstellt) werden beim nächsten `swayg`-Aufruf erfasst ("lazy sync").

```
swayg CLI
---------
1. sync_from_sway()          — neue Workspaces in DB aufnehmen, gelöschte entfernen
2. Command ausführen          — DB Write + Sway IPC (rename, focus, etc.)
3. sync_all_suffixes()       — Suffixe anpassen
```

## Änderungen

### Phase 1: Daemon entfernen

- `swaygd` binary entfernen (`sway-groups-cli/src/bin/daemon.rs`)
- `swaygd.service` löschen
- `install.sh`: `swaygd` nicht mehr installieren, systemd service nicht mehr aktivieren
- `commands.rs`: `DaemonAction` und `run_daemon()` entfernen
- `CLI_SPEC.md` aktualisieren

### Phase 2: Lazy Sync

Jeder `swayg`-Aufruf beginnt mit `sync_from_sway()`:

1. **Neue Workspaces erfassen**:
   - Alle Sway-Workspaces holen
   - Für jeden: Suffix strippen -> base_name
   - Wenn base_name nicht in DB: neuen Workspace-Eintrag erstellen
   - Gruppe ermitteln: focused workspace -> zu welcher Gruppe gehört er? -> diese Gruppe
   - Fallback: wenn kein workspace focused -> Gruppe `"0"`

2. **Gelöschte Workspaces entfernen**:
   - Alle DB-Workspaces holen
   - Für jeden: existiert er noch in Sway?
   - Wenn nein: Workspace aus DB löschen
   - Auch `group_state` Einträge (last_focused_workspace) bereinigen

3. **SQLite WAL Mode**:
   - Bei DB-Öffnung: `PRAGMA journal_mode=WAL` setzen
   - Erlaubt concurrent Readers + single Writer ohne Blocking
   - Verhindert "database is locked" bei schnellen aufeinanderfolgenden CLI-Aufrufen (Key-Repeat)

### Phase 3: sync_from_sway() erweitern

- Aktuelle Implementierung erstellt neue Workspaces, entfernt aber keine gelöschten
- Erweitern um Cleanup-Schritt (gelöschte Workspaces aus DB entfernen)
- `group_state` Einträge referenzieren workspaces by ID -> bei Löschung auch diese referenzen bereinigen

### Phase 4: Aktive Gruppe für neue Workspaces

- Statt "0" als Default-Gruppe: den focused workspace ermitteln
- Seine Gruppe aus der DB lesen (workspace -> workspace_group -> group)
- Neuer Workspace wird in diese Gruppe aufgenommen
- Fallback: kein focused workspace -> Gruppe `"0"`

### Phase 5: Aufräumen

- `swaygd-protocol` crate entfernen (wurde im Redesign erstellt)
- Unnötige Dependencies entfernen (tokio für CLI? Nur noch für IPC)
- Integrationstests aktualisieren (kein Daemon-Start/Stop mehr nötig)
- `REDESIGN_PLAN.md` (alter Plan) entfernen

### Phase 6: Testen

- Integrationstests laufen lassen
- Manuell testen: Key-Repeat, schnelle aufeinanderfolgende Aufrufe
- Externe Workspace-Erstellung testen (z.B. `swaymsg workspace 99` -> `swayg sync`)

## Was bleibt

- `sway-groups-core`: Geschäftslogik (Services, Entities, DB)
- `sway-groups-cli`: CLI binary (`swayg`)
- `swayg sync` command für manuellen sync
- Skripte in `scripts/` (rofi)
