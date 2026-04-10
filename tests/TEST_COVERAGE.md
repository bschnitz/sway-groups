# Test Coverage — Ungetestete Commands

## Status: Alle bestehenden Tests (13) = 343/343 PASS

---

## Commands & Testabdeckung

### `group` Subcommands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `group list` | PARTIAL | (implizit in vielen Tests via `workspace list`) | Output-Format nicht explizit verifiziert |
| `group create` | IMPLIZIT | test01 (via `--create` flag) | Nie als eigenständiger Befehl getestet |
| `group delete` | PARTIAL | test06a | Ohne `--force` nur 1 Assertion. Ohne Workspace-Mitglieder nicht getestet. Orphan-Cleanup (WS nicht in sway) nicht getestet. |
| `group rename` | **NEIN** | — | Komplett ungetestet |
| `group select` | JA | test01, alle 05x, 06x | Gut abgedeckt inkl. `--create` |
| `group active` | IMPLIZIT | test01 (Setup) | Nie als eigenständiger Befehl mit Assertion getestet |
| `group next` | **NEIN** | — | Komplett ungetestet |
| `group prev` | **NEIN** | — | Komplett ungetestet |
| `group next-on-output` | **NEIN** | — | Komplett ungetestet |
| `group prev-on-output` | **NEIN** | — | Komplett ungetestet |
| `group prune` | **NEIN** | — | Komplett ungetestet |

### `workspace` Subcommands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `workspace list` | PARTIAL | test03, 05x, 06x | `--visible`, `--plain`, `--group`, `--output` teilweise. Status-Ausgabe `(global)`, `(visible)`, `(hidden)` nicht verifiziert. |
| `workspace add` | JA | test02, test05a | Gut abgedeckt |
| `workspace move` | JA | test06b | `--groups` getestet |
| `workspace remove` | JA | test05f | Aus Gruppe entfernen getestet |
| `workspace rename` | JA | test05c | Merge-Szenario getestet. Einfaches Rename (ohne Merge) nicht getestet. |
| `workspace global` | JA | test03, test05d | Gut abgedeckt |
| `workspace unglobal` | JA | test05e | Gut abgedeckt |
| `workspace groups` | **NEIN** | — | Komplett ungetestet |

### `nav` Subcommands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `nav next` | **NEIN** | — | Komplett ungetestet |
| `nav prev` | **NEIN** | — | Komplett ungetestet |
| `nav next-on-output` | **NEIN** | — | Komplett ungetestet |
| `nav prev-on-output` | **NEIN** | — | Komplett ungetestet |
| `nav go` | **NEIN** | — | Komplett ungetestet |
| `nav move-to` | **NEIN** | — | Komplett ungetestet |
| `nav back` | **NEIN** | — | Komplett ungetestet |

### `container` Subcommands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `container move` | JA | test02, test04, test05b | Mit und ohne `--switch-to-workspace` getestet |

### Top-Level Commands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `init` | IMPLIZIT | alle Tests | Jeder Test beginnt mit `init` |
| `sync` | **NEIN** | — | Weder `--all`, `--workspaces`, `--groups`, noch `--outputs` separat getestet |
| `repair` | **NEIN** | — | Komplett ungetestet |
| `status` | **NEIN** | — | Komplett ungetestet |

---

## Priorisierte Testpläne

### P1 — Kritisch (Core-Funktionalität)

1. **test07_group_rename.sh** — `group rename`: einfaches Rename, Rename mit Output-Referenz-Update, Rename mit group_state-Update, Fehlerfälle (existiert nicht, Zielname existiert, Gruppe "0")
2. **test08_nav_next_prev.sh** — `nav next`, `nav prev`: Navigieren zwischen sichtbaren Workspaces, Wrap-Verhalten, Boundary (letztes/erstes WS ohne Wrap)
3. **test09_nav_go_back.sh** — `nav go`, `nav back`: Gehe zu Workspace, gehe zurück, keine Historie
4. **test10_workspace_rename_simple.sh** — `workspace rename` ohne Merge (einfaches Umbenennen)

### P2 — Wichtig (Randfälle & Data Integrity)

5. **test11_group_delete_edge_cases.sh** — `group delete`: leere Gruppe, WS nur in DB (nicht in sway), Gruppe "0" darf nicht gelöscht werden
6. **test12_group_next_prev.sh** — `group next`, `group prev`, `group next-on-output`, `group prev-on-output`: Wrap, Boundary
7. **test13_repair.sh** — `repair`: Stale Workspaces in DB, fehlende Workspaces in DB, leere Gruppen
8. **test14_workspace_list_output_format.sh** — `workspace list`: `(global)`, `(visible)`, `(hidden)` Status-Markierung
9. **test15_workspace_groups.sh** — `workspace groups`: Zeigt korrekte Gruppen-Mitgliedschaften

### P3 — Nice-to-have

10. **test16_group_prune.sh** — `group prune`: `--keep` Flag
11. **test17_sync_flags.sh** — `sync --workspaces`, `sync --outputs`, `sync --groups`
12. **test18_status.sh** — `status`: Output-Format
13. **test19_group_create.sh** — `group create` als eigenständiger Befehl (Fehlerfall: existiert bereits)
14. **test20_nav_move_to.sh** — `nav move-to`

---

## Fortschritt

| # | Test | Status |
|---|------|--------|
| 01 | test01_group_select.sh | DONE (9/9) |
| 02 | test02_new_workspace.sh | DONE (25/25) |
| 03 | test03_global_workspace.sh | DONE (30/30) |
| 04 | test04_workspace_move.sh | DONE (19/19) |
| 05a | test05a_multi_group_workspace_add.sh | DONE (26/26) |
| 05b | test05b_multi_group_container_move.sh | DONE (32/32) |
| 05c | test05c_multi_group_workspace_rename_merge.sh | DONE (44/44) |
| 05d | test05d_multi_group_global.sh | DONE (22/22) |
| 05e | test05e_multi_group_unglobal.sh | DONE (29/29) |
| 05f | test05f_multi_group_workspace_remove.sh | DONE (28/28) |
| 05g | test05g_multi_group_auto_delete.sh | DONE (27/27) |
| 06a | test06a_group_delete_multi_group_workspace.sh | DONE (30/30) |
| 06b | test06b_workspace_move_to_groups.sh | DONE (22/22) |
| 07 | test07_group_rename.sh | TODO |
| 08 | test08_nav_next_prev.sh | TODO |
| 09 | test09_nav_go_back.sh | TODO |
| 10 | test10_workspace_rename_simple.sh | TODO |
| 11 | test11_group_delete_edge_cases.sh | TODO |
| 12 | test12_group_next_prev.sh | TODO |
| 13 | test13_repair.sh | TODO |
| 14 | test14_workspace_list_output_format.sh | TODO |
| 15 | test15_workspace_groups.sh | TODO |
| 16 | test16_group_prune.sh | TODO |
| 17 | test17_sync_flags.sh | TODO |
| 18 | test18_status.sh | TODO |
| 19 | test19_group_create.sh | TODO |
| 20 | test20_nav_move_to.sh | TODO |
