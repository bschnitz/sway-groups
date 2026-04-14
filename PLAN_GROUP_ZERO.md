# Plan: Gruppe "0" — Refactor zu Optional

## Ziel

Gruppe "0" soll **nur dann existieren**, wenn sie wirklich gebraucht wird:
- **a) Fresh init**: Wenn noch keine andere Gruppe existiert
- **b) Repair**: Verwaiste Workspaces brauchen eine Gruppe

Gruppe "0" wird ansonsten wie jede andere Gruppe behandelt — kein Sonderstatus, kein Auto-Delete-Schutz, kein Visibility-Catch-All.

## Kernidee

`active_group` in der `outputs` Tabelle wird **nullable** (`Option<String>`). Der Wert `None` bedeutet "keine Gruppe aktiv", nicht "Gruppe 0".

## Phasen

### Phase 1: DB Schema

**`outputs.active_group` → nullable**

- `output.rs:16`: `active_group: String` → `active_group: Option<String>`
- `database.rs`: Schema-Migration bestehender DBs:
  ```sql
  -- Bei init (DB wird neu erstellt): kein Problem
  -- Bei bestehender DB: ALTER TABLE outputs ALTER COLUMN active_group DROP NOT NULL;
  ```
  Da `swayg init` die DB löscht und neu erstellt, wird `sea-orm`'s Schema-Creation das nullable Field automatisch anlegen. Kein Migration-Script nötig.

- `find_by_active_group` in `output.rs`: Parameter `&Option<String>` statt `&str`

### Phase 2: Entity + Service — "0" Fallbacks → None

Alle Stellen die `"0"` als Default liefern, stattdessen `None`:

**GroupService:**
- `get_active_group()` (implizit via OutputEntity): Gibt jetzt `Option<String>` statt `String` zurück. Alle Caller anpassen.
- `set_active_group()`:
  - Zeile 422: `old_group = "0"` → `old_group = None`
  - Zeile 428: `old_group_needs_cleanup = old_group != group && old_group != "0"` → `old_group_needs_cleanup = old_group.is_some() && old_group != Some(group.clone())`
- `group_next()`, `group_prev()`, `group_create()`, `group_rename()`, `group_delete()`:
  - Alle `get_active_group().unwrap_or_else(|_| "0".to_string())` → `get_active_group().unwrap_or(None)`
- `prune()` Zeile 706: `if group.name == "0"` → entfernen (Gruppe "0" kann auch gepruned werden)
- `ensure_default_group()`: **komplett entfernen** — Gruppe "0" wird nicht mehr proaktiv erstellt

**WorkspaceService:**
- Zeile 40, 526: `active_group` Fallback `"0"` → `None`
- Zeile 79: `active_group == "0"` catch-all → `active_group.is_none()` catch-all (d.h. "kein Filter")
- Zeile 732, 893: `active_group: Set("0".to_string())` bei Output-Sync → `active_group: Set(None)`
- Zeile 950: `GroupEntity::find_by_name("0")` im repair → beibalten (Fall a)

**NavigationService:**
- Zeile 25, 64, 98, 137, 403, 476: Alle `"0"` Fallbacks → `None`
- Zeile 64, 137: `active_group == "0"` catch-all → `active_group.is_none()`

**VisibilityService:**
- Zeile 76, 147: `active_group == "0"` catch-all → `active_group.is_none()`
- Zeile 164: `"0"` Fallback → `None`

**WaybarSyncService:**
- Zeile 54, 94, 134, 135: Alle `"0"` Fallbacks → `None`
- Zeile 94: `memberships.is_empty() && active_group == "0"` → `active_group.is_none()`

### Phase 3: CLI — "0" nur bei Bedarf erstellen

**Entfernen:**
- `main.rs:60`: `group_service.ensure_default_group().await?` → entfernen
- `commands.rs:740`: `group_service.ensure_default_group().await?` in sync → entfernen
- `commands.rs:796`: `group_svc.ensure_default_group().await?` in init → **ändern**: Nur erstellen wenn nach init keine Gruppe existiert (Fall b)
- `commands.rs:826`: `group_service.ensure_default_group().await?` in repair → **ändern**: Nur erstellen wenn repair orphans produziert und keine andere Gruppe existiert (Fall a)

**Init (commands.rs):**
```rust
// Nach sync_from_sway():
if group_svc.list_groups().await?.is_empty() {
    // Fall b: Frisches System, keine Gruppen → erstelle "0" als Default
    group_svc.create_group("0").await?;
    group_svc.set_active_group("0", &output).await?;
}
```

**Repair:**
```rust
// Nach repair:
let orphans = ...; // workspaces die keiner Gruppe angehören
if orphans > 0 {
    let groups = group_svc.list_groups().await?;
    let target = if groups.iter().any(|g| g.name == "0") {
        "0"
    } else {
        group_svc.create_group("0").await?;
        "0"
    };
    // orphans to target group...
}
```

### Phase 4: Visibility — None bedeutet "alle sichtbar"

Wenn `active_group` = `None`:
- `is_workspace_visible()`: Alle Workspaces sind sichtbar (wie heute bei `"0"`)
- `nav next/prev`: Zyklisch durch ALLE Workspaces des Outputs
- `workspace list`: Alle Workspaces des Outputs zeigen

### Phase 5: Gruppe "0" Sonderbehandlung entfernen

**GroupService:**
- Zeile 144: `if name == "0"` (delete-Schutz) → entfernen
- Zeile 248: `if old_name == "0"` (rename-Schutz) → entfernen
- Zeile 428: `old_group != "0"` (auto-delete-Schutz) → entfernen (Phase 2 bereits geändert)

**GroupEntity:**
- Zeile 42-44: `has_default_group()` → entfernen

### Phase 6: set_active_group — Workspace "0" in sway

Beim Wechsel in eine leere Gruppe wird sway's workspace `"0"` fokussiert. Das bleibt, aber:
- Der workspace `"0"` wird in die ZIELGRUPPE eingetragen (nicht in Gruppe "0")
- `ensure_workspace_in_group("0", group, output)` bleibt (das ist sway's ws "0", nicht die DB-Gruppe "0")

### Phase 7: Tests anpassen

- Tests die `fixture.swayg(&["group", "select", "0", ...])` nutzen: Prüfen ob "0" noch erstellt wird bei init. Wenn ja, funktioniert es weiter. Wenn nein, muss der Test ggf. `--create` nutzen.
- test_01 etc.: `group select "0"` als "zurück zum Default" funktioniert nur wenn "0" existiert. Alternativ: `group select` ohne Argument?
- Post-conditions: `"0"` ist kein Sonderfall mehr, kann auch gelöscht/gepruned werden

## Risiken

1. **`swayg group active` liefert None**: CLI muss das handhaben (leere Ausgabe oder "none")
2. **Waybar-Groups-Widget**: `None` als aktive Gruppe → Widget zeigt keine Gruppe als aktiv (oder zeigt "(none)")
3. **Daemon**: `get_active_group()` liefert None → externe Workspaces werden keiner Gruppe zugewiesen. Das ist OK — beim nächsten `group select` oder `init` werden sie einer Gruppe zugewiesen.
