# Plan: waybar-dynamic Integration

## Ziel

Die `_class_hidden` / `_class_global` suffixes werden abgeschafft. Statt sway
workspace-Namen zu ändern (`rename workspace`), wird die Anzeige über
[waybar-dynamic](https://github.com/…) gesteuert. swayg sendet einen JSON-Payload
an einen Unix Socket, waybar-dynamic rendert die Widgets.

Vorteile:
- Kein workspace-rename → kein flicker, kein feedback-loop
- Ein einzelner IPC-Aufruf statt mehrere `swaymsg rename workspace`
- CSS-Klassen für Styling (`.active`, `.global`, `.visible`, …)
- waybar-dynamic module name: `swayg_workspaces`

## waybar-dynamic IPC (Kurzfassung)

- Socket: `$XDG_RUNTIME_DIR/waybar-dynamic-swayg_workspaces.sock`
- Protokoll: newline-delimited JSON
- Operationen: `set_all`, `patch`, `clear`
- WidgetSpec Felder: `id`, `label`, `classes`, `tooltip`, `on_click`

```json
{
    "op": "set_all",
    "widgets": [
        { "id": "ws-1", "label": "1",   "classes": ["focused"],  "on_click": "swaymsg workspace 1" },
        { "id": "ws-3", "label": "3",   "classes": [],           "on_click": "swaymsg workspace 3" },
        { "id": "ws-5", "label": "5",   "classes": ["global"],   "on_click": "swaymsg workspace 5" }
    ]
}
```

## Phasen

### Phase 1: `WaybarClient`

Neues Modul: `sway-groups-core/src/sway/waybar_client.rs`

- UnixStream-Client für `$XDG_RUNTIME_DIR/waybar-dynamic-swayg_workspaces.sock`
- `XDG_RUNTIME_DIR` aus Umgebungsvariable lesen
- Methoden:
  - `new() -> Result<Self>` — baut Socket-Pfad, prüft ob Socket existiert
  - `send_set_all(widgets: &[WidgetSpec]) -> Result<()>`
  - `send_clear() -> Result<()>`
  - `send(payload: &IpcMessage) -> Result<()>` — generisch
- Fehlerbehandlung: Socket nicht erreichbar → log Warnung, kein Panic
  (waybar-dynamic muss nicht zwingend laufen)
- JSON-Strukturen:
  - `IpcMessage { op: String, widgets: Option<Vec<WidgetSpec>>, ... }`
  - `WidgetSpec { id: String, label: String, classes: Vec<String>, on_click: String }`

### Phase 2: `WaybarSyncService`

Neuer Service: `sway-groups-core/src/services/waybar_sync_service.rs`

Ersetzt `SuffixService` als primärer Sync-Mechanismus.

- `new(db, ipc_client, waybar_client) -> Self`
- `update_waybar() -> Result<()>` — Hauptfunktion:
  1. Alle outputs von sway holen
  2. Für jeden output: `active_group` aus DB
  3. Alle sway-workspaces durchlaufen, visibility berechnen (gleiche Logik
     wie bisher in `calculate_suffix`, aber ohne suffix):
     - workspace in aktiver group → `classes: ["focused"]` wenn focused, sonst `[]`
     - workspace global → `classes: ["global"]` (+ "focused" wenn focused)
     - workspace in anderer group → **nicht aufnehmen** (hidden)
     - workspace ohne group + active_group == "0" → aufnehmen
    4. Widgets nach workspace-nummer/name sortieren
    5. `on_click`: `swaymsg workspace "<name>"`
    6. Einziger `send_set_all()` Aufruf
- `update_waybar_for_output(output_name: &str) -> Result<()>` — optional,
  falls pro Output sync nötig wird (zunächst nicht verwendet)

### Phase 3: Integration in bestehende Services

Alle Aufrufe von `suffix_service.sync_all_suffixes()` werden ersetzt durch
`waybar_sync.update_waybar()`.

**Betroffene Dateien:**

| Datei | Funktion | Änderung |
|---|---|---|
| `commands.rs` | `run_nav()` | `suffix_service.sync_all_suffixes()` → `waybar_sync.update_waybar()` |
| `commands.rs` | `run_workspace()` (add/move/remove/global/unglobal) | gleich |
| `commands.rs` | `run_sync()` | gleich |
| `commands.rs` | `run_status()` | DB-basierte Anzeige statt suffix-basiert |
| `group_service.rs` | `set_active_group()` | `suffix_service.sync_all_suffixes()` → `waybar_sync.update_waybar()` |
| `navigation_service.rs` | `resolve_sway_workspace_name()` | suffix-resolve-Logik entfernen, workspace-Namen sind jetzt immer die echten Namen |

**`resolve_sway_workspace_name()`** vereinfachen:
- Nur noch exact match gegen sway workspace names
- Kein `get_base_name()` / suffix-stripping mehr nötig
- Fallback: Name as-is (sway erstellt workspace)

**`SuffixService`**: Vorerst bestehen lassen, wird nicht mehr aktiv verwendet.
Kann in einem späteren Schritt komplett entfernt werden.

### Phase 4: `run_status()` anpassen

Statt suffix-basierte Sichtbarkeitsprüfung (`is_hidden()`, `is_global()` via
workspace name suffix), jetzt DB-basiert:

- `is_global`: aus `workspace.is_global` in DB
- visibility: gleiche Logik wie `get_visible_workspaces()` (group membership
  + active_group Vergleich)

### Phase 5: CLI `main.rs` / Service-Initialisierung

- `WaybarClient` instanziieren (besteht sich, auch wenn socket nicht da)
- `WaybarSyncService` erstellen und an Commands übergeben
- `SuffixService` wird aus den Command-Parametern entfernt (bzw. nur noch
  für legacy `strip_suffix` behalten)

### Phase 6: Integrationstests aktualisieren

- Suffix-basierte Assertions entfernen (kein `_class_hidden` / `_class_global`
  mehr in sway workspace names)
- `get_base_name()` helper im Test-Script kann vereinfacht werden
- Test ob swayg ohne laufenden waybar-dynamic nicht crashed (socket nicht da)
- `run_nav` tests: prüfen dass workspace-Namen sauber sind (keine suffixes)

### Phase 7: Aufräumen (später)

- `SuffixService` komplett entfernen (calculate_suffix, apply_suffix,
  sync_suffixes_for_output, sync_all_suffixes, SUFFIX_HIDDEN, SUFFIX_GLOBAL)
- `workspace_service::strip_suffix()` entfernen (nur noch nötig wenn alte
  suffixes noch in DB/sway existieren)
- `navigation_service::suffix_service` Feld entfernen

## Keine Migration

Bestehende `_class_*` suffixes werden nicht automatisch aus sway entfernt.
Das kann bei Bedarf später manuell passieren (einmaliger `swayg sync --all`
der alle workspaces renamt, oder ein `swayg migrate` Kommando).

## waybar Config (nur zur Info)

In der waybar config des Users muss der waybar-dynamic module konfiguriert
sein:

```jsonc
{
    "modules-left": ["cffi/swayg_workspaces"],
    "cffi/swayg_workspaces": {
        "module_path": "~/.config/waybar/modules/libwaybar_dynamic.so",
        "name": "swayg_workspaces",
        "spacing": 0
    }
}
```

swayg konfiguriert waybar nicht — das ist Aufgabe des Users.
