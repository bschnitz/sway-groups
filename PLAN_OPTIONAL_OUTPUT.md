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

### Phase 1: CLI + Core (Rust)

#### 1.1 `commands.rs` — CLI Definition

**`GroupAction::Select`** (Zeile 73-78):
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

**`GroupAction::Next`** (Zeile 82-87) und **`GroupAction::Prev`** (Zeile 94-99):
Bereits `Option<String>` — keine Änderung nötig an der Definition.

#### 1.2 `commands.rs` — Neuer Helper: `resolve_group_output`

Neue Funktion, die den Output für eine Gruppe ermittelt:

```rust
fn resolve_group_output(
    explicit_output: Option<&str>,
    group: &str,
    db: &DatabaseManager,
    ipc_client: &SwayIpcClient,
) -> anyhow::Result<String> {
    if let Some(output) = explicit_output {
        return Ok(output.to_string());
    }

    // Try group_state for last-visited output
    // (sqlite3 query oder via entity)
    let last_output = /* query group_state ORDER BY last_visited DESC LIMIT 1 */;

    if let Some(output) = last_output {
        return Ok(output);
    }

    // Fallback: current focused output
    ipc_client.get_primary_output()
}
```

Dafür muss der `DatabaseManager` (bzw. die DB Connection) im CLI Handler verfügbar sein. Prüfen ob das schon der Fall ist.

**Alternative:** Die Logik direkt in `set_active_group` in group_service.rs einbauen statt im CLI Handler. Dann braucht der CLI Handler nur den Parameter zu ändern und weiterzugeben.

#### 1.3 `commands.rs` — CLI Handler

**`GroupAction::Select`** (Zeile 274-285):
```rust
// VON:
GroupAction::Select { output, group, create } => {
    ...
    group_service.set_active_group(&output, &group).await?;
    ...
}

// AUF:
GroupAction::Select { output, group, create } => {
    ...
    let resolved_output = resolve_group_output(output.as_deref(), &group, ...)?;
    group_service.set_active_group(&resolved_output, &group).await?;
    ...
}
```

**`GroupAction::Next`** und **`GroupAction::Prev`**:
```rust
// VON:
GroupAction::Next { output, wrap } => {
    let output = resolve_output(output.as_deref(), ipc_client)?;
    ...
    group_service.next_group(&output, wrap).await?;
}

// AUF:
GroupAction::Next { output, wrap } => {
    // output is None -> resolve_output gives current output
    let output = resolve_output(output.as_deref(), ipc_client)?;
    ...
    // next_group returns the group name, but we need to resolve its output
    if let Some(next) = group_service.next_group(&output, wrap).await? {
        let resolved_output = resolve_group_output(None, &next, ...)?;
        group_service.set_active_group(&resolved_output, &next).await?;
        ...
    }
}
```

**Achtung:** Bei `group next`/`group prev` ist die Logik komplexer, da die Zielgruppe erst nach dem Aufruf von `next_group`/`prev_group` bekannt ist. Zwei Ansätze:

- **Ansatz A:** `next_group`/`prev_group` geben nur den Gruppennamen zurück (wie bisher), dann wird der Output für diese Gruppe aufgelöst und `set_active_group` aufgerufen. Problem: `next_group` ruft intern bereits `set_active_group` auf (Rekursion!).
- **Ansatz B:** `next_group`/`prev_group` bekommen einen Parameter `auto_resolve_output: bool`. Wenn `true`, lösen sie den Output intern auf.

**Empfehlung:** Ansatz A ist sauberer. `next_group`/`prev_group` sollten nicht `set_active_group` intern aufrufen, sondern nur den Gruppennamen zurückgeben. Der CLI Handler macht dann die Output-Auflösung und ruft `set_active_group` selbst. Das bedeutet aber Refactoring in group_service.rs.

#### 1.4 `group_service.rs` — `next_group` / `prev_group` Refactoring

Aktuell rufen `next_group` und `prev_group` intern `set_active_group` auf (Zeilen 516, 538, 560, 582). Das ist ein Problem für die output-übergreifende Variante, weil der Output erst nach der Gruppenauswahl feststeht.

**Lösung:** Neue Methoden `next_group_name` und `prev_group_name` die nur den Gruppennamen zurückgeben, ohne `set_active_group` aufzurufen. Die CLI Handler verwenden diese und übergeben das Ergebnis an `set_active_group`.

Alternativ: Bestehende Methoden umbauen, sodass sie optionales Output-Argument akzeptieren und den Output intern auflösen.

#### 1.5 `group_state.rs` — Neue Query

Methode zum Finden des letzten Outputs für eine Gruppe:

```rust
pub fn find_last_visited_output(group_name: &str) -> ... {
    // SELECT output FROM group_state
    // WHERE group_name = ?
    // ORDER BY last_visited DESC
    // LIMIT 1
}
```

### Phase 2: Scripts (2 Dateien)

#### 2.1 `scripts/rofi-group-select`

Zeile 25:
```bash
# VON:
$HOME/.cargo/bin/swayg group select "$output" "$group" --create

# AUF:
$HOME/.cargo/bin/swayg group select "$group" --create
# (kein --output nötig, wird automatisch aufgelöst)
```

Das Script kann auch die Output-Ermittlung am Anfang vereinfachen/entfernen.

#### 2.2 `scripts/rofi-workspace-move`

Zeile 36:
```bash
# VON:
$HOME/.cargo/bin/swayg group select "$current_output" "$first_group"

# AUF:
$HOME/.cargo/bin/swayg group select "$first_group" --output "$current_output"
```

(Hier explizit `--output` weil wir auf dem aktuellen Output bleiben wollen, nicht zum letzten Output der Gruppe springen.)

### Phase 3: Tests (~19 Dateien, ~50+ Aufrufe)

#### 3.1 `group select` Aufrufe

Alle Tests müssen von:
```bash
$SG group select $OUTPUT '$GROUP'
```
auf:
```bash
$SG group select '$GROUP' --output $OUTPUT
```
geändert werden.

**Betroffene Testdateien:**
- test01_group_select.sh (9 Aufrufe)
- test02_new_workspace.sh (4)
- test03_global_workspace.sh (6)
- test04_workspace_move.sh (5)
- test05a_multi_group_workspace_add.sh (7)
- test05b_multi_group_container_move.sh (7)
- test05c_multi_group_workspace_rename_merge.sh (7)
- test05d_multi_group_global.sh (3)
- test05e_multi_group_unglobal.sh (6)
- test05f_multi_group_workspace_remove.sh (7)
- test05g_multi_group_auto_delete.sh (11)
- test06a_group_delete_multi_group_workspace.sh (8)
- test06b_workspace_move_to_groups.sh (10)
- test08_nav_next_prev.sh (4)
- test09_nav_go_back.sh (4)
- test10_workspace_rename_simple.sh (4)
- test11_workspace_groups.sh (5)
- test12_repair.sh (2)
- test13_group_next_prev.sh (4)
- test14_workspace_list_output_format.sh (4)
- test15_sync_flags.sh (4)
- test16_group_prune.sh (4)
- test17_status.sh (4)
- test19_nav_move_to.sh (1)

**Alle `group select` mit `--create` sind auch betroffen:**
```bash
# VON:
$SG group select $OUTPUT '__test_group__' --create

# AUF:
$SG group select '__test_group__' --output $OUTPUT --create
```

#### 3.2 `group next` / `group prev` Aufrufe

`test13_group_next_prev.sh` — bereits `--output $OUTPUT` Syntax (Zeilen 126, 132, 138, 144, 150, 156, 162, 168, 177, 183). **Keine Änderung nötig.**

#### 3.3 Neue Tests

Test für output-übergreifendes Verhalten (nur wenn `--output` weggelassen wird):
- `group select <group>` ohne `--output` → wechselt zum letzten Output der Gruppe
- `group next` ohne `--output` → wechselt zur nächsten Gruppe auf deren letztem Output
- Edge case: Gruppe die noch nie auf einem Output besucht wurde → Fallback auf aktuellen Output

Diese Tests erfordern **zwei Outputs** (z.B. virtueller Output oder zwei physische Outputs). Auf einem Single-Output-System können sie nicht vollständig getestet werden. Vermutlich auf später verschieben.

### Phase 4: Dokumentation

#### 4.1 `tests/test-instructions.md`

- Regel 28 (aktuell): `swayg group select <OUTPUT> <GROUP> --create`
  → Anpassen an neue Syntax: `swayg group select <GROUP> --output <OUTPUT> --create`
- Mehrere Beispiele im Template (Zeilen 37, 72, 108, 127, 180, 193)

#### 4.2 `README.md` und `CLI_SPEC.md`

- Command-Syntax für `group select`, `group next`, `group prev` aktualisieren

#### 4.3 Rust Integration Tests

- `sway-groups-tests/tests/test01_group_select.rs` (Zeilen 39, 52) — `set_active_group` Aufrufe. Diese testen die Service-Schicht direkt, nicht die CLI. **Nicht betroffen** wenn sich die Service-Signatur nicht ändert.

## Ausführungsreihenfolge

1. **Phase 1.5:** `group_state.rs` — neue Query Methode
2. **Phase 1.2:** `commands.rs` — neuer Helper `resolve_group_output`
3. **Phase 1.1:** `commands.rs` — CLI Definition ändern
4. **Phase 1.3:** `commands.rs` — CLI Handler anpassen
5. **Phase 1.4:** `group_service.rs` — `next_group`/`prev_group` Refactoring
6. **Build & Test:** `cargo build --release`, `install.sh`, `tests/run_all.sh`
7. **Phase 3:** Tests anpassen (sed/replace)
8. **Phase 2:** Scripts anpassen
9. **Phase 4:** Dokumentation aktualisieren
10. **Commit**

## Offene Fragen

- Soll `next_group`/`prev_group` intern `set_active_group` aufrufen (aktuell) oder nur den Namen zurückgeben (Refactoring)? Letzteres ist sauberer für output-übergreifendes Verhalten, aber ein größerer Umbau.
- Sollen `next-on-output` und `prev-on-output` ebenfalls output-optional werden, oder bleiben diese output-spezifisch? (Aktuelle Annahme: bleiben so.)
- Wie testen wir output-übergreifendes Verhalten ohne zweiten physischen Output?
