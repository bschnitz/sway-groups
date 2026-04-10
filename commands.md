swayg group
- list [-o OUTPUT] — Gruppen und Workspaces anzeigen
- create NAME / delete NAME [-f] / rename OLD NEW
- select OUTPUT GROUP — aktive Gruppe setzen
- active OUTPUT — aktuelle Gruppe anzeigen
- next/prev [-o OUTPUT] [-w] — Gruppe wechseln
- next-on-output/prev-on-output [-w] — nächste nicht-leere Gruppe
- prune [--keep NAME...] — leere Gruppen löschen
swayg workspace
- list [-o OUTPUT] [-g GROUP] [--visible] [--plain]
- add WS [-g GROUP] / remove WS [-g GROUP] / move WS --groups G1,G2
- rename OLD NEW — umbenennen (oder merge wenn Ziel existiert)
- global WS / unglobal WS / groups WS
swayg nav
- next/prev [-o OUTPUT] [-w] — nächsten/vorherigen Workspace
- next-on-output/prev-on-output [-w] — global über alle Outputs
- go WS — zu Workspace navigieren
- move-to WS — Container verschieben (fügt Ziel zur aktiven Gruppe hinzu)
- back — zurück zum vorherigen Workspace
swayg sync
- --all / --workspaces / --groups / --outputs
swayg init — DB plattmachen und neu initialisieren
swayg status — aktive Gruppen und sichtbare/versteckte Workspaces pro Output
