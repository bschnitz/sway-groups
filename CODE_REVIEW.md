# Code Review: sway-groups

Stand: 2026-04-14
Analysiert mit: Claude Code (Sonnet 4.6)

---

## Zusammenfassung

Das Projekt ist solide strukturiert: klare Crate-Trennung, konsequente Service-Layer-Architektur, gute Fehlertypen und ein umfangreiches Integrations-Test-Setup. Die folgenden Punkte sind Verbesserungsvorschläge – keine Showstopper. Sie sind nach Priorität und Kategorie gegliedert.

---

## 1. Datenbankzugriff: N+1 Query Problem

**Wo:** `group_service.rs` → `list_groups()`, `is_effectively_empty()`, `ensure_workspace_in_group()`;
`workspace_service.rs` → `list_workspaces()`, `list_visible_workspaces()`;
`visibility_service.rs` → `get_visible()`, `get_visible_for_group()`

**Problem:** Für jedes Element in einer Schleife werden weitere DB-Queries ausgeführt.

```rust
// list_groups(): Für jede Gruppe → alle Memberships → für jede Membership → ein Workspace
for group in groups {
    let memberships = WorkspaceGroupEntity::find_by_group(group.id).all(...).await?;
    for membership in memberships {
        if let Some(ws) = WorkspaceEntity::find_by_id(membership.workspace_id).one(...).await? {
            // ...
        }
    }
}
```

Bei 20 Gruppen mit je 10 Workspaces = ~200+ Queries pro `list_groups()`-Aufruf.

**Lösung:** JOIN-Queries oder Batch-Loads nutzen. Sea-ORM unterstützt `find_with_related` und `LoaderTrait` für diesen Zweck.

---

## 2. Duplizierte Visibility-Logik

**Wo:** `visibility_service.rs::get_visible()` und `workspace_service.rs::list_visible_workspaces()`

**Problem:** Beide Methoden enthalten nahezu identischen Code (ca. 40 Zeilen) zur Bestimmung sichtbarer Workspaces:

```rust
// In beiden Services identisch:
if workspace.is_global { ... continue; }
let memberships = WorkspaceGroupEntity::find_by_workspace(workspace.id)...;
for m in &memberships {
    if active_group == group.name { visible.push(...); break; }
}
if !found && memberships.is_empty() && active_group.is_none() { ... }
```

**Lösung:** `WorkspaceService::list_visible_workspaces()` sollte intern `VisibilityService::get_visible()` delegieren oder beide nutzen eine gemeinsame private Hilfsfunktion.

---

## 3. Fehlende Datenbank-Transaktionen

**Wo:** `group_service.rs::rename_group()`, `delete_group()`, `set_active_group()`, `ensure_workspace_in_group()`

**Problem:** Zusammengehörige Operationen laufen ohne Transaktion. Bei einem Fehler nach dem ersten Schritt bleibt die DB in einem inkonsistenten Zwischenzustand.

```rust
// rename_group(): 3 separate Updates ohne Transaktion
group.update(...).await?;                          // Schritt 1
for output in affected_outputs { output.update(...).await?; }  // Schritt 2
for state in affected_states { state.update(...).await?; }     // Schritt 3
```

**Lösung:** Sea-ORM's `db.transaction(|txn| { ... }).await?` für alle Multi-Step-Operationen nutzen.

---

## 4. `active_group` als String statt Foreign Key

**Wo:** `entities/output.rs` (Feld `active_group: Option<String>`), `entities/group_state.rs` (Feld `group_name: String`)

**Problem:** Diese Felder speichern Gruppennamen statt IDs. Damit gibt es:
- Keine referentielle Integrität auf Datenbankebene
- `rename_group()` muss manuell alle betroffenen Zeilen suchen und aktualisieren (fehlerträchtig)
- Inkonsistenz möglich, wenn eine der Aktualisierungen fehlschlägt

**Lösung:** `group_id: Option<i32>` statt `active_group: Option<String>`. Das würde `rename_group()` erheblich vereinfachen und DB-Konsistenz garantieren. (Erfordert Migration.)

**Alternativer Kurzweg:** Zumindest DB-seitige `ON UPDATE CASCADE` / `ON DELETE SET NULL` Trigger nutzen.

---

## 5. Keine DB-Migrations-Strategie

**Wo:** `db/database.rs`

**Problem:** Schema-Erstellung nutzt nur `if_not_exists` über Sea-ORM's `Schema::create_table_from_entity`. Das bedeutet: neue Spalten bei Schema-Änderungen werden **nicht automatisch hinzugefügt**. Bestehende DBs werden nicht migriert.

```rust
let mut stmt = schema.create_table_from_entity(GroupEntity);
stmt.if_not_exists();  // Erstellt nur, migriert nie
conn.execute(&stmt).await?;
```

Das `schema-sync` Feature in `Cargo.toml` ist aktiviert, aber nicht genutzt.

**Lösung:** Migrations-Tool einsetzen (z.B. `sea-orm-migration` oder `sqlx::migrate!`) mit versionierten SQL-Dateien. Alternativ: beim Start die Schema-Version prüfen und Upgrade-Queries ausführen.

---

## 6. Service-Konstruktion: Redundanter Config-Pattern

**Wo:** `GroupService`, `WorkspaceService`, `NavigationService`, `VisibilityService` – alle vier haben `new()` + `with_config()` und tragen `default_group`/`default_workspace` als Felder.

**Problem:**
- Die gleichen Felder (`default_group`, `default_workspace`) werden 4x redundant gehalten
- `new()` hardcodiert `"0"` als Default – das kann vom Produktions-Config abweichen
- Inkonsistenz-Risiko: Wenn zwei Services mit unterschiedlicher Config erstellt werden

**Lösung:** Entweder die Config als Arc in alle Services injizieren, oder einen `ServiceContext`-Struct einführen, der DB, IPC-Client und Config bündelt und nur einmal erstellt wird:

```rust
pub struct ServiceContext {
    pub db: DatabaseManager,
    pub ipc: SwayIpcClient,
    pub config: Arc<SwaygConfig>,
}
```

---

## 7. Keine Abstraktion für SwayIpcClient (Testbarkeit)

**Wo:** Alle Services

**Problem:** `SwayIpcClient` ist ein konkreter Typ. Services sind damit eng an eine echte Sway-Verbindung gekoppelt. Unit-Tests sind faktisch unmöglich – daher gibt es nur Integrationstests, die Sway laufen brauchen.

**Lösung:** Trait einführen:

```rust
pub trait SwayClient: Send + Sync {
    fn get_workspaces(&self) -> Result<Vec<Workspace>>;
    fn get_outputs(&self) -> Result<Vec<Output>>;
    fn run_command(&self, cmd: &str) -> Result<Vec<CommandResult>>;
    // ...
}
```

`SwayIpcClient` implementiert den Trait. Tests können dann einen `MockSwayClient` injizieren. Das würde auch Unit-Tests für komplexe Logik wie `set_active_group()` oder `should_delete_old_group()` ermöglichen.

---

## 8. `update_active_group_quiet`: Naming und Code-Duplizierung

**Wo:** `group_service.rs:556`

**Probleme:**
- Parameter heißen `_output` und `_group` (Underscore-Prefix = "absichtlich unbenutzt") – aber sie werden benutzt
- Der Output-Upsert-Code ist identisch zu `set_active_group()` (~30 Zeilen dupliziert)
- "quiet" ist als Name unpräzise; gemeint ist "only-db, no sway focus"

**Lösung:**
1. Parameter umbenennen: `_output` → `output`, `_group` → `group`
2. Upsert-Logik in `upsert_output_active_group(output, group)` extrahieren
3. Methode umbenennen: z.B. `set_active_group_db_only()`

---

## 9. `GroupInfo.workspace_count` ist redundant

**Wo:** `group_service.rs:12`

```rust
pub struct GroupInfo {
    pub workspace_count: usize,   // == workspaces.len()
    pub workspaces: Vec<String>,
}
```

`workspace_count` ist immer `workspaces.len()`. Das ist eine Inkonsistenz-Quelle und unnötiger State.

**Lösung:** Feld entfernen; Aufrufer nutzen `info.workspaces.len()`.

---

## 10. Inkonsistente Fehlerbehandlung: `anyhow` vs. `thiserror`

**Wo:** `db/database.rs` nutzt `anyhow::Result`, alle Services nutzen den eigenen `Result`-Alias.

**Problem:** `DatabaseManager::new()` gibt `AnyResult<Self>` zurück. Der Aufrufer in `main.rs` muss dann von `anyhow::Error` in den eigenen Fehlertyp konvertieren – oder die Fehlergrenze ist verwischt.

**Lösung:** Konsistent `crate::error::Result` / `Error` im gesamten Core nutzen. `database.rs` sollte `Error::Database` oder `Error::Io` zurückgeben statt `anyhow::Error`.

---

## 11. Doppeltes Logging bei Fehlern

**Wo:** `group_service.rs:178`

```rust
warn!("Group '{}' has {} workspaces. Use --force to delete anyway.", name, memberships.len());
return Err(Error::InvalidArgs(format!(
    "Group '{}' has {} workspaces. Use --force to delete anyway.", name, memberships.len()
)));
```

Dieselbe Nachricht wird als Log-Warning und als Error-String zurückgegeben. Der Aufrufer loggt den Error möglicherweise nochmals.

**Lösung:** Nur `return Err(...)` – der Aufrufer entscheidet, ob er loggt.

---

## 12. WAL-Modus wird zu spät gesetzt

**Wo:** `db/database.rs:69`

```rust
// Nach der Schema-Erstellung:
conn.execute_unprepared("PRAGMA journal_mode=WAL").await.ok();
```

WAL sollte als erstes gesetzt werden, bevor irgendwelche Tabellen angelegt werden. Außerdem wird der Fehler mit `.ok()` ignoriert – WAL ist für Concurrent-Access aber wesentlich.

**Lösung:** PRAGMA vor den CREATE TABLE Statements; Fehler nicht ignorieren.

---

## 13. `is_empty() == false` statt `!is_empty()`

**Wo:** `group_service.rs:486`

```rust
if ws.name == dw && ws.output != output && ws.output.is_empty() == false {
```

**Lösung:** `!ws.output.is_empty()` – Standard-Rust-Idiom.

---

## 14. Inkonsistente Einrückung in `database.rs`

**Wo:** `db/database.rs:58–66`

Die letzten zwei `GroupState`- und `PendingWorkspaceEvent`-Blöcke haben eine andere Einrückung als die vorherigen Blöcke (4 Spaces vs. 8 Spaces).

---

## 15. `next_group` / `prev_group` Methoden-Explosion

**Wo:** `group_service.rs:598–713`

Vier sehr ähnliche Methoden: `next_group`, `next_group_on_output`, `prev_group`, `prev_group_on_output`. Jede hat eine `*_name`-Variante. Das sind 8 öffentliche Methoden, die alle das gleiche Muster haben: resolve output → get list → compute index → switch.

**Lösung:** Den gemeinsamen Kern extrahieren:

```rust
enum Direction { Next, Prev }
enum Scope { AllGroups, OutputOnly }

async fn navigate_group(&self, output: &str, dir: Direction, scope: Scope, wrap: bool) -> Result<Option<String>>
```

---

## 16. Fehlende Validierung: Leerstrings

**Wo:** `group_service.rs::create_group()`, `workspace_service.rs::add_workspace()`

Gruppe und Workspace können mit leerem Namen oder Whitespace-only Namen erstellt werden. Das führt zu Darstellungsproblemen in der Bar und potenziell zu Sway-IPC-Fehlern.

**Lösung:** Am Eintrittspunkt validieren:

```rust
if name.trim().is_empty() {
    return Err(Error::InvalidArgs("Name darf nicht leer sein".into()));
}
```

---

## 17. `BarSectionConfig::default()` vs. `SwaygConfig::default()` Inkonsistenz

**Wo:** `sway-groups-config/src/lib.rs:80–89` vs. `62–78`

`BarSectionConfig::default()` setzt `socket_instance: String::new()` (leer), aber `SwaygConfig::default()` setzt konkrete Werte wie `"swayg_workspaces"`. Wenn `BarSectionConfig::default()` direkt aufgerufen wird (z.B. in Tests), erhält man einen invaliden Config-Zustand.

**Lösung:** Die struct-level `Default`-Implementierung entfernen oder sinnvolle Defaults setzen. `#[serde(default)]` auf Feldebene mit `default_fn`-Attributen nutzen.

---

## 18. `config_path()` nutzt falsche Organization-ID

**Wo:** `sway-groups-config/src/lib.rs:93`

```rust
let dirs = directories::ProjectDirs::from("com", "swayg", "swayg")?;
```

`"com"` als Qualifier macht nur bei reversed domain names Sinn (z.B. `"io"`, `"org"`). Für ein lokales CLI-Tool ist der Qualifier typischerweise leer. Das führt zu einem Config-Pfad wie `~/.config/com.swayg.swayg/` statt `~/.config/swayg/`.

**Lösung:** `ProjectDirs::from("", "", "swayg")` oder direkt `dirs::config_dir().map(|d| d.join("swayg").join("config.toml"))`.

*(Prüfen, wo die Config tatsächlich landet – wenn Nutzer sie schon angelegt haben, ist eine Migration nötig.)*

---

## Zusammenfassung nach Priorität

| Priorität | Thema |
|-----------|-------|
| Hoch | N+1 Queries (#1) |
| Hoch | Fehlende Transaktionen (#3) |
| Hoch | `active_group` als String statt FK (#4) |
| Hoch | Keine Migrations-Strategie (#5) |
| Mittel | Duplizierte Visibility-Logik (#2) |
| Mittel | Service-Konstruktion redundant (#6) |
| Mittel | Keine SwayClient-Abstraktion (#7) |
| Mittel | `update_active_group_quiet` Naming/Duplizierung (#8) |
| Mittel | Inkonsistente Fehlerbehandlung anyhow vs. thiserror (#10) |
| Niedrig | `GroupInfo.workspace_count` redundant (#9) |
| Niedrig | Doppeltes Logging (#11) |
| Niedrig | WAL-Timing (#12) |
| Niedrig | `is_empty() == false` (#13) |
| Niedrig | Einrückung database.rs (#14) |
| Niedrig | Methoden-Explosion next/prev group (#15) |
| Niedrig | Fehlende Leerstring-Validierung (#16) |
| Niedrig | BarSectionConfig Default-Inkonsistenz (#17) |
| Niedrig | config_path Organization-ID (#18) |
