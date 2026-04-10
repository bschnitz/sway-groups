# Test Coverage — All Commands

## Status: All 19 tests passing (643 assertions)

---

## Commands & Testabdeckung

### `group` Subcommands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `group list` | PARTIAL | (implizit in vielen Tests) | Output-Format nicht explizit verifiziert |
| `group create` | JA | test18 | Erfolg + Fehler (existiert bereits) |
| `group delete` | JA | test06a | Mit `--force`, orphan cleanup |
| `group rename` | JA | test07 | Einfach + Output-Ref-Update + group_state-Update + Fehlerfälle |
| `group select` | JA | test01, alle 05x, 06x | Inkl. `--create` |
| `group active` | IMPLIZIT | test01 (Setup) | Nie als eigenständiger Befehl mit Assertion getestet |
| `group next` | JA | test13 | Wrap, Boundary |
| `group prev` | JA | test13 | Wrap, Boundary |
| `group next-on-output` | JA | test13 | |
| `group prev-on-output` | JA | test13 | |
| `group prune` | JA | test16 | `--keep` Flag |

### `workspace` Subcommands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `workspace list` | JA | test14 | `(global)`, `(visible)`, `(hidden)`, `--visible`, `--plain`, `--group`, `--output` |
| `workspace add` | JA | test02, test05a | |
| `workspace move` | JA | test06b | `--groups` getestet |
| `workspace remove` | JA | test05f | |
| `workspace rename` | JA | test10, test05c | Einfach + Merge |
| `workspace global` | JA | test03, test05d | |
| `workspace unglobal` | JA | test05e | |
| `workspace groups` | JA | test11 | |

### `nav` Subcommands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `nav next` | JA | test08 | Wrap, Boundary |
| `nav prev` | JA | test08 | Wrap, Boundary |
| `nav next-on-output` | JA | test08 | |
| `nav prev-on-output` | JA | test08 | |
| `nav go` | JA | test09 | |
| `nav move-to` | JA | test19 | Auto-DB-Sync für neue Workspaces |
| `nav back` | JA | test09 | |

### `container` Subcommands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `container move` | JA | test02, test04, test05b | Mit und ohne `--switch-to-workspace` |

### Top-Level Commands

| Command | Getestet | Test-Datei | Notizen |
|---------|----------|------------|---------|
| `init` | IMPLIZIT | alle Tests | Jeder Test beginnt mit `init` |
| `sync` | JA | test15 | `--workspaces`, `--outputs`, `--groups` |
| `repair` | JA | test12 | |
| `status` | JA | test17 | Output-Format, hidden/global |

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
| 07 | test07_group_rename.sh | DONE (29/29) |
| 08 | test08_nav_next_prev.sh | DONE (37/37) |
| 09 | test09_nav_go_back.sh | DONE (25/25) |
| 10 | test10_workspace_rename_simple.sh | DONE (23/23) |
| 11 | test11_workspace_groups.sh | DONE (20/20) |
| 12 | test12_repair.sh | DONE (22/22) |
| 13 | test13_group_next_prev.sh | DONE (37/37) |
| 14 | test14_workspace_list_output_format.sh | DONE (33/33) |
| 15 | test15_sync_flags.sh | DONE (26/26) |
| 16 | test16_group_prune.sh | DONE (35/35) |
| 17 | test17_status.sh | DONE (11/11) |
| 18 | test18_group_create.sh | DONE (13/13) |
| 19 | test19_nav_move_to.sh | DONE (16/16) |

## Ungetestete Randfälle

- `group active` als eigenständiger Befehl (immer nur via Setup genutzt)
- `group list` Output-Format (implizit geprüft)
- `container move` mit `--switch-to-workspace` auf einen neuen Workspace
