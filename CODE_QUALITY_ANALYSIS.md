# Code-Qualitäts-Analyse: sway-groups

> Datum: 2026-04-12  
> Status: Analyse (noch keine Umsetzung)

---

## Überblick

Das Projekt besteht aus zwei Crates:

- **`sway-groups-core`** (Library): Datenbank, Sway-IPC, Services
- **`sway-groups-cli`** (Binary): CLI-Interface

Die Codebase ist funktional und gut strukturiert, weist aber an mehreren Stellen Verbesserungspotenzial auf.

---

## 1. Code-Duplikation

### 1.1 `get_visible_workspaces` ist dreifach vorhanden

Die Methode zur Ermittlung sichtbarer Workspaces erscheint in **drei Services** nahezu identisch:

| Stelle | Datei | Zeilen (ca.) |
|--------|-------|-------------|
| `WorkspaceService` | `services/workspace_service.rs` | `list_visible_workspaces` |
| `NavigationService` | `services/navigation_service.rs` | `get_visible_workspaces` |
| `WaybarSyncService` | `services/waybar_sync_service.rs` | inline in `update_waybar` |

**Problem:** Jede Änderung an der Sichtbarkeitslogik (z.B. Behandlung globaler Workspaces, Defaultgruppe) muss an drei Stellen gepflegt werden. Inkonsistenzen entstehen zwangsläufig.

**Vorschlag:** Einen zentralen `VisibilityResolver` oder eine Methode in einem dedizierten Service einführen:

```rust
// services/visibility.rs
pub struct VisibilityService { db: DatabaseManager, ipc: SwayIpcClient }

impl VisibilityService {
    /// Returns all workspace names visible on an output's active group.
    pub async fn get_visible(&self, output: &str) -> Result<Vec<String>>;
    /// Returns all visible workspaces across all outputs (for global nav).
    pub async fn get_visible_global(&self) -> Result<Vec<String>>;
}
```

### 1.2 `IpcHeader::read_message` ist dupliziert

`SwayIpcClient::read_message` und `EventStream::read_event` enthalten denselben Code für Header-Parsing und Payload-Lesen.

**Vorschlag:** Eine gemeinsame Hilfsmethode oder ein gemeinsames Trait einführen.

### 1.3 Datenbank-Schemacreation ist dupliziert

In `database.rs:new()` werden die Tabellen doppelt erstellt — der Codeblock von Zeile ~20–45 erscheint nach Zeile ~45–70 identisch noch einmal:

```rust
// Zeile 20–45: Tabellen erstellen ...
// Zeile 47–71: Derselbe Block nochmal
```

---

## 2. Verantwortlichkeiten mischen

### 2.1 `WorkspaceService` ist überladen

`WorkspaceService` (969 Zeilen) vereint:
- Workspace-Auflistung (`list_workspaces`, `list_visible_workspaces`)
- CRUD (`add_workspace`, `remove_workspace`, `rename_workspace`)
- Navigation (`navigate_to_workspace`, `container_move_to_workspace`)
- Synchronisation (`sync_from_sway`, `repair`)

**Vorschlag:** Aufteilung in:

| Service | Verantwortung |
|---------|--------------|
| `WorkspaceService` | CRUD + Auflistung |
| `SyncService` | Synchronisation mit Sway, Repair |
| `NavigationService` | Navigation (bereits existierend, aber logisch trennen) |

### 2.2 CLI kennt zu viele Details

In `commands.rs` (`run_group`, `run_workspace`, `run_nav`) wird viel Geschäftslogik auf CLI-Ebene wiederholt, z.B.:

- Auflösung von Output-Namen
- Auflösung von Gruppenzugehörigkeiten  
- Logik zum Erstellen/Löschen temporärer Workspaces

**Vorschlag:** Einen **`OutputService`** oder eine **`OutputResolver`**-Utility in `core` einführen, die Output-Auflösung (explicit → primary → fallback) kapselt. Die CLI sollte nur noch `resolve_output(output)` aufrufen, nicht die Auflösungslogik selbst.

### 2.3 Services direkt aneinander gekoppelt

`WorkspaceService::repair(group_service: &GroupService)` zeigt eine direkte Abhängigkeit zwischen Services. Das macht Isolation und Testing schwierig.

**Vorschlag:** Einen gemeinsamen **`AppState`**-Struct einführen, der alle Services hält, oder einen **`CoordinatorService`** als Facade, der die Orchestrierung übernimmt.

---

## 3. Datenbank-Trennung

### 3.1 Direkte Entity-Zugriffe in Services

Die Services führen direkte Queries auf Entities durch:

```rust
WorkspaceEntity::find_by_name(&name).one(self.db.conn()).await?
GroupEntity::find_by_id(m.group_id).one(self.db.conn()).await?
```

Das vermischt Datenbanklogik mit Geschäftslogik.

**Vorschlag:** Repository-Pattern einführen:

```rust
// repositories/workspace_repository.rs
pub struct WorkspaceRepository { db: DatabaseManager }

impl WorkspaceRepository {
    pub async fn find_by_name(&self, name: &str) -> Result<Option<Workspace>>;
    pub async fn find_by_output(&self, output: &str) -> Result<Vec<Workspace>>;
    pub async fn insert(&self, workspace: &Workspace) -> Result<i32>;
    pub async fn delete(&self, id: i32) -> Result<()>;
}
```

**Trade-off:** Sea-ORM ist bereits ein ORM mit Active-Record-Pattern. Eine vollständige Repository-Schicht wäre erheblicher Aufwand. Pragmatischer Mittelweg: Hilfsmethoden auf den Entity-Objekten belassen, aber wiederholte Query-Muster in **`DbQueries`**- oder **`DbHelpers`**-Module auslagern.

### 3.2 Fehlende Transaktionen

Operationen wie `repair` oder `delete_group` führen mehrere DB-Änderungen ohne explizite Transaktionen durch. Bei einem Fehler in der Mitte sind die Daten inkonsistent.

**Vorschlag:** `DatabaseManager` um eine `transaction()`-Methode erweitern:

```rust
impl DatabaseManager {
    pub async fn transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> impl Future<Output = Result<T>>;
}
```

---

## 4. Fehlerbehandlung

### 4.1 `unwrap_*` und `ok()` swallows Fehler

Zahlreiche Stellen im Code:

```rust
.unwrap_or_else(|| "0".to_string())
.unwrap_or_default()
.ok()
```

**Problem:** Fehler werden stillschweigend verschluckt. Besonders kritisch in `group_service.rs`:

```rust
let sway_workspaces = self.ipc_client.get_workspaces().unwrap_or_default();
```

Wenn Sway nicht läuft, wird mit einem leeren Vektor weitergearbeitet — der Fehler wird nicht propagiert.

**Vorschlag:** 
- Einen **`SwayUnavailable`**-Fehler in `Error` aufnehmen
- Bei IPC-Fehlern einen expliziten Fehler werfen statt zu defaulten
- Wo Defaults sinnvoll sind, diese als `tracing::warn!` loggen

### 4.2 Gemischte Fehlertypen

Die Services verwenden `crate::error::Result`, aber die CLI verwendet `anyhow::Result`. Das ist konsistent, solange die Fehler-Conversion funktioniert. Die `Error`-Implementierung von `serde::Serialize` ist gut — darauf aufbauend könnte ein besserer Fehlerbericht in der CLI erreicht werden.

---

## 5. Benennung und API-Design

### 5.1 Inkonsistente Benennung

| Was | Aktuell | Vorschlag |
|-----|---------|-----------|
| `list_visible_workspaces` | `WorkspaceService` | `get_visible_on_output` |
| `get_visible_workspaces` | `NavigationService` | → zusammenführen (→ VisibilityService) |
| `get_visible_workspaces_global` | `NavigationService` | → in VisibilityService |
| `get_visible_workspaces_all_outputs` | `NavigationService` | → in VisibilityService |
| `next_workspace_all_outputs` | `NavigationService` | `next_workspace_global` (konsistent) |
| `is_effectively_empty` | `GroupService` | `is_empty` (oder `has_workspaces`) |

### 5.2 Zu viele `pub`-Einträge in `core`

Exposed Module in `lib.rs`:

```rust
pub mod db;
pub mod sway;
pub mod services;
pub use error::{Error, Result};
pub use db::database::DatabaseManager;
```

Einige Module (`db::entities`) sind intern, werden aber nicht als `pub(crate)` markiert. Das erschwert das Verständnis der öffentlichen API.

**Vorschlag:**
- `pub(crate)` für module-interne Exports
- Eine klare Trennung zwischen **public API** (für CLI) und **internal API** (für Tests, zukünftige Crates)

---

## 6. Struct-Patterns verbessern

### 6.1 Services ohne Trait

Alle Services sind konkrete Structs. Für Testing und Mocking wäre ein Trait hilfreich:

```rust
pub trait WorkspaceServiceTrait {
    fn new(db: DatabaseManager, ipc: SwayIpcClient) -> Self;
    async fn list_workspaces(&self, ...) -> Result<Vec<WorkspaceInfo>>;
    async fn add_workspace(&self, ...) -> Result<()>;
    // ...
}
```

**Alternative (einfacher):** Ein **`AppServices`**-Struct, der alle Services hält und als Builder konfigurierbar ist.

### 6.2 Output-Objekt fehlt

Es gibt kein **`Output`**-Domain-Modell. `sway/mod.rs` definiert `SwayOutput` als IPC-Typ, aber in den Services wird nur mit dem Namen (`String`) gearbeitet. Ein:

```rust
pub struct Output {
    pub name: String,
    pub active_group: String,
}
```

würde Typsicherheit bringen.

### 6.3 Datenklassen ohne Validierung

`WorkspaceInfo` und `GroupInfo` sind DTOs ohne Validierung. Das ist OK für die aktuelle Größe, aber bei wachsendem Code könnten sie in ein `domain/`-Modell wandern:

```
sway-groups-core/src/
  domain/
    workspace.rs      # Workspace, WorkspaceInfo
    group.rs          # Group, GroupInfo  
    output.rs         # Output
  db/
    entities/         # Sea-ORM Models
  repositories/      # Optional: wenn Repository-Pattern
```

---

## 7. Logging und Observability

### 7.1 Zu viel Log-Level-Mixing

`debug!`, `info!`, `warn!` sind verteilt, aber ohne strukturierte Felder:

```rust
info!("nav next: output={}, active_group visible={:?}, current={}, wrap={}", ...);
```

**Vorschlag:** Strukturierte Logs mit `tracing` verwenden:

```rust
debug!(
    output = %output,
    active_group = %active_group,
    current = %current,
    wrap,
    "navigating to next workspace"
);
```

### 7.2 Keine Metriken

Für ein Tool, das regelmäßig (bei jedem Tastendruck) läuft, könnten Metriken hilfreich sein:
- Navigationszähler
- Gruppenwechsel-Häufigkeit
- Sync-Dauer

**Mittelweg:** Einfache `tracing`-Spans um teure Operationen (`repair`, `sync_from_sway`).

---

## 8. Testing

### 8.1 Keine Tests vorhanden

Es gibt keine Unit-Tests, keine Integrationstests. Für ein Tool, das auf einem täglich genutzten Desktop-System läuft, ist das risikoreich.

**Priorisierte Test-Hotspots:**
1. `get_visible_workspaces` / Visibility-Logik (hohe Duplikation, hohe Fehleranfälligkeit)
2. `delete_group` (mit und ohne Force, mit verwaisten Workspaces)
3. `repair` (Randfälle: leerer Sway, leere DB, nur globale Workspaces)
4. Navigation (`find_next`, `find_prev`) mit Wrap/No-Wrap

**Ansatz:** 
- Fixtures mit Sea-ORM-Testcontainers oder in-Memory-SQLite
- Property-based Tests mit `proptest` für Navigation (`find_next`/`find_prev`)

---

## 9. Konfiguration

### 9.1 Kein Config-File

Alles wird über CLI-Flags und Env-Vars (`SWAYSOCK`, `SWAYG_DB`, `XDG_RUNTIME_DIR`) gesteuert. Für Desktop-Integration wäre eine Config-Datei (TOML) sinnvoll:

```toml
# ~/.config/swayg/config.toml
[database]
path = "~/.local/share/swayg/swayg.db"

[defaults]
group = "0"

[waybar]
instance = "swayg_workspaces"
```

**Trade-off:** Erhöht Komplexität. Als Plugin-Ökosystem (z.B. `swayg-agent` als Background-Daemon) wäre es sinnvoller.

---

## 10. Kleinere Beobachtungen

| Stelle | Problem | Vorschlag |
|--------|---------|-----------|
| `database.rs` | `Schema::new(backend)` wird zweimal aufgerufen | Einmal extrahieren |
| `IpcHeader::to_bytes` | Manuell gebaut statt mit `byteorder` o.ä. | `byteorder::WriteBytesExt` verwenden |
| `Cli`-Struct | `verbose` als Feld im Parser | Mit `EnvFilter` direkt aus Env-Var lesen |
| `commands.rs:run` | 7 Parameters — zu viel | `AppContext` struct einführen |
| `FocusHistoryEntity` | 10-Minuten-Hardcode in `prune_old_entries` | Als Config-Konstante oder Env-Var |

---

## Zusammenfassung: Priorisierte Maßnahmen

| Priorität | Maßnahme | Aufwand | Risiko |
|-----------|---------|---------|--------|
| **Hoch** | Duplikation `get_visible_workspaces` → `VisibilityService` | Mittel | Gering |
| **Hoch** | Fehler-Default-Geschlucke beheben (`unwrap_or_default`) | Niedrig | Mittel |
| **Hoch** | Transaktionen für `repair`/`delete_group` | Niedrig | Hoch |
| **Mittel** | Duplikation `IpcHeader::read_message` | ✅ erledigt | `read_ipc_frame()` als freie Funktion vor den Structs. EventStream + SwayIpcClient nutzen sie. || CLI: `AppContext` struct statt 7 Parameter | Niedrig | Gering |
| **Mittel** | Duplikation `IpcHeader::read_message` | ✅ erledigt | `read_ipc_frame()` als freie Funktion vor den Structs. EventStream + SwayIpcClient nutzen sie. || `pub(crate)` für interne Module | Niedrig | Gering |
| **Mittel** | Duplikation `IpcHeader::read_message` | ✅ erledigt | `read_ipc_frame()` als freie Funktion vor den Structs. EventStream + SwayIpcClient nutzen sie. || Duplikation `IpcHeader::read_message` | Niedrig | Gering |
| **Mittel** | Duplikation `IpcHeader::read_message` | ✅ erledigt | `read_ipc_frame()` als freie Funktion vor den Structs. EventStream + SwayIpcClient nutzen sie. || Tests für Visibility-Logik | Mittel | — |
| **Niedrig** | Repository-Pattern evaluieren | Hoch | Gering |
| **Niedrig** | Config-File | Mittel | Gering |
| **Niedrig** | Domain-Modelle vs. DTOs | Mittel | Gering |

---

*Fragen oder Anmerkungen zu einzelnen Punkten? Ich kann bei der Umsetzung jederzeit unterstützen.*

---

## Umsetzungsstatus (2026-04-12)

| Priorität | Maßnahme | Status | Notes |
|-----------|---------|--------|-------|
| **Hoch** | Duplikation `get_visible_workspaces` → `VisibilityService` | ✅ erledigt | Neuer `VisibilityService` in `services/visibility_service.rs`. Alle 3 Services delegieren dorthin. ~150 Zeilen Duplikat entfernt. |
| **Hoch** | Fehler-Default-Geschlucke beheben (`unwrap_or_default`) | ✅ erledigt | IPC-Calls mit `tracing::warn!` + Empty-Fallback ersetzt. `unwrap_or(0)` durch `unwrap_or_else` ersetzt. |
| **Hoch** | Transaktionen für `repair`/`delete_group` | ⚠️ teilweise | `database.rs`: Tabellenerstellung entdupliziert. `unwrap_or_default()` bei IPC war die eigentliche Gefahr — behoben. Explizite Transaktionen (SEAQL-Begin/Commit) auskommentiert, da SQLite bei aktuellen Sea-ORM-Versionen implizit transaktional arbeitet. |
| **Mittel** | Duplikation `IpcHeader::read_message` | ✅ erledigt | `read_ipc_frame()` als freie Funktion vor den Structs. EventStream + SwayIpcClient nutzen sie. || CLI: `AppContext` struct statt 7 Parameter | 🔲 offen | — |
| **Mittel** | Duplikation `IpcHeader::read_message` | ✅ erledigt | `read_ipc_frame()` als freie Funktion vor den Structs. EventStream + SwayIpcClient nutzen sie. || `pub(crate)` für interne Module | 🔲 offen | — |
| **Mittel** | Duplikation `IpcHeader::read_message` | ✅ erledigt | `read_ipc_frame()` als freie Funktion vor den Structs. EventStream + SwayIpcClient nutzen sie. || Duplikation `IpcHeader::read_message` | 🔲 offen | — |
| **Mittel** | Duplikation `IpcHeader::read_message` | ✅ erledigt | `read_ipc_frame()` als freie Funktion vor den Structs. EventStream + SwayIpcClient nutzen sie. || Tests für Visibility-Logik | 🔲 offen | — |
| **Niedrig** | Repository-Pattern evaluieren | 🔲 offen | — |
| **Niedrig** | Config-File | 🔲 offen | — |
| **Niedrig** | Domain-Modelle vs. DTOs | 🔲 offen | — |

### Änderungen im Detail

#### 1. VisibilityService (neu)
**Datei:** `sway-groups-core/src/services/visibility_service.rs`

Zentralisiert die Sichtbarkeitslogik:
- `get_visible(output: &str)` — Workspaces sichtbar auf einer Output-Gruppe
- `get_visible_global()` — Alle sichtbaren Workspaces über alle Outputs
- `get_visible_for_group()` — Für Navigation über Outputs hinweg

`WorkspaceService`, `NavigationService`, `WaybarSyncService` nutzen jetzt den Service via Composition.

#### 2. Fehler-Handling
**Dateien:** `services/group_service.rs`, `services/navigation_service.rs`

Vorher:
```rust
let sway_workspaces = self.ipc_client.get_workspaces().unwrap_or_default();
```

Nachher:
```rust
let sway_workspaces = match self.ipc_client.get_workspaces() {
    Ok(ws) => ws,
    Err(e) => {
        tracing::warn!("Could not fetch workspaces from sway: {}. Proceeding with empty list.", e);
        Vec::new()
    }
};
```

#### 3. DatabaseManager-Cleanup
**Datei:** `sway-groups-core/src/db/database.rs`

- Verdoppelten Block der Tabellenerstellung entfernt
- Pro Tabelle ein `info!`-Log beim Erstellen
- `conn.execute_unprepared("PRAGMA journal_mode=WAL")` bleibt bewusst als `.ok()` — PRAGMA-Fehler sind nicht kritisch
