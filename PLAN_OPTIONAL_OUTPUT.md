# Plan: Optional Output für `group select`, `group next`, `group prev`

## Ziel

`output` wird von einem **positional required** Parameter zu einem **optionalen `--output` Flag**.

- **Ohne `--output`**: Der Output wird automatisch aus `group_state` (letzter Besuch) ermittelt. Fallback: aktueller Output (der des fokussierten Workspace).
- **Mit `--output`**: Verhalten wie bisher.

Dies betrifft drei Commands:
1. `swayg group select <group> [--output <output>] [--create]`
2. `swayg group next [--output <output>] [--wrap]`
3. `swayg group prev [--output <output>] [--wrap]`

**Nicht betroffen:** `group next-on-output` und `group prev-on-output` — diese sind bereits output-spezifisch und bleiben so.

## Hintergrund: Output-Auflösung

Wenn `--output` nicht angegeben wird, muss der "richtige" Output für eine Gruppe ermittelt werden. Die Logik:

1. Für die Zielgruppe: `SELECT output FROM group_state WHERE group_name = '<group>' ORDER BY last_visited DESC LIMIT 1;`
2. Falls kein Eintrag existiert: Fallback auf den Output des aktuell fokussierten Workspace (`get_primary_output()`).

**Wichtig:** `set_active_group` ruft intern `focus_workspace(ws_name)` auf, was in sway automatisch den Output wechselt, auf dem der Workspace liegt. Der Output-Wechsel passiert also implizit über sway.

## Änderungen

### Phase 1: Tests (zuerst!)

#### 1.1 Neue Tests: Output-übergreifendes Verhalten

Alle neuen Tests nutzen einen virtuellen Output (`swaymsg create_output HEADLESS-1`, Cleanup: `swaymsg output HEADLESS-1 unplug`).

| Test | Datei | Beschreibung | Virtueller Output |
|------|--------|-------------|:---:|
| **test_20** | `test_20_optional_output_select_auto_resolve.rs` | Gruppe auf eDP-1 besucht, Test auf virtuellem Output, `group select <group>` (kein `--output`) → wechselt zu eDP-1. Verifiziert Output + Workspace + active group. | Ja |
| **test_21** | `test_21_optional_output_next_prev_auto_resolve.rs` | Zwei Gruppen auf verschiedenen Outputs besucht. `group next`/`group prev` ohne `--output` → Zielgruppe auf deren letztem Output. Cross-Output-Wechsel verifiziert. | Ja |
| **test_22** | `test_22_optional_output_fallback.rs` | Neue Gruppe erstellt (nie besucht, kein `group_state` Eintrag). `group select <group>` ohne `--output` → bleibt auf aktuellem Output (Fallback). | Ja |

**Abgedeckte Szenarien:**
- A) Auto-Resolve via `group_state` (test_20, test_21)
- B) Fallback bei fehlendem `group_state` (test_22)
- C) Cross-Output-Wechsel (test_20, test_21)
- D) `--output` Override → wird durch bestehende Tests abgedeckt (Phase 1.2)

**WICHTIG:** Diese Tests werden ZUERST geschrieben (TDD). Sie werden erwartet, dass sie fehlschlagen (rot), bevor die Implementation erfolgt.

#### 1.2 Bestehende Tests aktualisieren (21 Dateien, 85 Aufrufe)

Alle `group select` Aufrufe in den Rust-Tests müssen von:
```rust
.swayg(&["group", "select", &fixture.orig_output, GROUP, "--create"])
```
auf:
```rust
.swayg(&["group", "select", GROUP, "--output", &fixture.orig_output, "--create"])
```
geändert werden. Der `output` Parameter wird vom ersten positional zu einem `--output` Flag.

**Betroffene Dateien (85 Aufrufe):**
- test_05g (12), test_06b (10), test_06a (8), test_13 (7), test_03 (6),
- test_05e (5), test_05f (6), test_02 (4), test_15 (4), test_14 (4),
- test_05a (2), test_05b (2), test_05c (2), test_16 (3), test_05d (3),
- test_01 (2), test_11 (1), test_09 (1), test_08 (1), test_04 (1),
- test_12 (1), test_17 (1)

### Phase 2: Core (Rust)

#### 2.1 `group_state.rs` — Neue Query

Neue Query-Methode zum Finden des letzten Outputs für eine Gruppe:

```rust
pub fn find_last_visited_output_for_group(group_name: &str) -> Select<Self> {
    Self::find()
        .filter(Column::GroupName.eq(group_name))
        .order_by_desc(Column::LastVisited)
}
```

#### 2.2 `group_service.rs` — Neue Methode: `find_last_visited_output`

```rust
pub async fn find_last_visited_output(&self, group: &str) -> Result<Option<String>>
```

Query: `SELECT output FROM group_state WHERE group_name = ? ORDER BY last_visited DESC LIMIT 1`

#### 2.3 `group_service.rs` — `next_group` / `prev_group` Refactoring

**Entscheidung:** Neue Methoden `next_group_name`/`prev_group_name` die nur den Gruppennamen zurückgeben, ohne `set_active_group` aufzurufen. Die bestehenden Methoden bleiben für `next-on-output`/`prev-on-output` wie sie sind.

Neue Methoden:
- `next_group_name(output, wrap) -> Result<Option<String>>` — nur Name, kein Switch
- `prev_group_name(output, wrap) -> Result<Option<String>>` — nur Name, kein Switch
- `next_group_on_output_name(output, wrap) -> Result<Option<String>>` — analog für on_output
- `prev_group_on_output_name(output, wrap) -> Result<Option<String>>` — analog für on_output

Bestehende `next_group`/`prev_group` intern umgebaut, um die neuen `*_name` Methoden zu nutzen.

#### 2.4 `commands.rs` — Neuer Helper: `resolve_group_output`

```rust
async fn resolve_group_output(
    explicit_output: Option<&str>,
    group: &str,
    group_service: &GroupService,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<String> {
    if let Some(output) = explicit_output {
        return Ok(output.to_string());
    }
    if let Some(output) = group_service.find_last_visited_output(group).await? {
        return Ok(output);
    }
    ipc_client.get_primary_output()
}
```

#### 2.5 `commands.rs` — CLI Definition ändern

**`GroupAction::Select`** (Zeile 77-82):
```rust
// VON:
Select {
    output: String,
    group: String,
    #[arg(short, long)]
    create: bool,
},

// AUF:
Select {
    group: String,
    #[arg(short, long)]
    output: Option<String>,
    #[arg(short, long)]
    create: bool,
},
```

**`GroupAction::Next`** und **`GroupAction::Prev`**: Bereits `Option<String>` — keine Änderung.

#### 2.6 `commands.rs` — CLI Handler

**`GroupAction::Select`**:
```rust
GroupAction::Select { output, group, create } => {
    let resolved_output = resolve_group_output(output.as_deref(), &group, ...).await?;
    group_service.set_active_group(&resolved_output, &group).await?;
}
```

**`GroupAction::Next`** und **`GroupAction::Prev`**:
```rust
GroupAction::Next { output, wrap } => {
    let current_output = resolve_output(output.as_deref(), ipc_client)?;
    if let Some(next_name) = group_service.next_group_name(&current_output, wrap).await? {
        let resolved_output = resolve_group_output(None, &next_name, ...).await?;
        group_service.set_active_group(&resolved_output, &next_name).await?;
    }
}
```

### Phase 3: Scripts (2 Dateien)

#### 3.1 `scripts/rofi-group-select`

```bash
# VON:
$HOME/.cargo/bin/swayg group select "$output" "$group" --create

# AUF:
$HOME/.cargo/bin/swayg group select "$group" --create
# (kein --output nötig, wird automatisch aufgelöst)
```

#### 3.2 `scripts/rofi-workspace-move`

```bash
# VON:
$HOME/.cargo/bin/swayg group select "$current_output" "$first_group"

# AUF:
$HOME/.cargo/bin/swayg group select "$first_group" --output "$current_output"
```

### Phase 4: Dokumentation

- `AI_TEST_INSTRUCTIONS.md` — Command-Syntax und Beispiele aktualisieren
- `CLI_SPEC.md` / `README.md` — falls vorhanden

## Ausführungsreihenfolge

1. **Phase 1.1:** Neue Tests schreiben (TDD — expected to fail)
2. **Phase 2.1:** `group_state.rs` — neue Query
3. **Phase 2.2:** `group_service.rs` — `find_last_visited_output`
4. **Phase 2.3:** `group_service.rs` — `next_group`/`prev_group` Refactoring
5. **Phase 2.5:** `commands.rs` — CLI Definition ändern
6. **Phase 2.4:** `commands.rs` — `resolve_group_output` Helper
7. **Phase 2.6:** `commands.rs` — CLI Handler anpassen
8. **Build:** `cargo build -p sway-groups-cli && bash install.sh`
9. **Phase 1.1:** Neue Tests laufen lassen (should pass now)
10. **Phase 1.2:** Bestehende Tests aktualisieren (85 Aufrufe)
11. **Run all tests:** `cargo test -p sway-groups-tests -- --test-threads=1`
12. **Phase 3:** Scripts anpassen
13. **Phase 4:** Dokumentation aktualisieren
14. **Commit**

## Offene Fragen

~~- Wie testen wir output-übergreifendes Verhalten ohne zweiten physischen Output?~~ → **Gelöst:** Virtueller Output via `swaymsg create_output HEADLESS-1`.

- `next-on-output`/`prev-on-output` bleiben output-spezifisch (keine Änderung).
