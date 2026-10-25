# tsess

Enregistre tes sessions terminal pour n'importe quel harness IA (Claude Code, Codex, Gemini CLI, opencode…) ou un simple shell, et rouvre-les plus tard pour continuer à travailler.

Deux binaires :
- `tsess` : la ligne de commande
- `tsess-bar` : une icône dans la barre de menus (macOS) ou la zone de notification (Linux, Windows)

## Installation

Il faut [Rust](https://rustup.rs).

```bash
cargo install --git https://github.com/seydou91/tsess
```

Sous Linux, installe d'abord : `sudo apt install libgtk-3-dev libxdo-dev libayatana-appindicator3-dev`

## Utilisation

```bash
tsess new audit -- claude   # lance claude dans le dossier courant et enregistre le terminal
tsess open audit            # retourne dans le dossier et reprend la session
tsess new                   # assistant : nom, commande, dossier
tsess ls                    # liste les sessions
tsess log audit             # relit l'enregistrement
tsess rm audit              # supprime la session
tsess-bar &                 # icône : clic sur une session pour la reprendre
```

Reprise selon le harness :

| Harness | Commande de reprise |
|---|---|
| claude | `claude --continue` |
| codex | `codex resume --last` |
| gemini | `gemini --resume latest` |
| opencode | `opencode --continue` |
| autre | relance la commande d'origine |

Les sessions sont dans `~/.tsess/`. Le menu de l'icône propose aussi « Nouvelle session… » et « Lancer au démarrage ».

## Limites

- `--continue` / `--last` reprennent la dernière session du dossier, pas forcément celle nommée si tu en as plusieurs dans le même dossier.
- Windows : pas d'enregistrement du terminal (`script` n'existe pas), seulement la reprise.
- Linux et Windows sont peu testés : les retours sont bienvenus.

## Licence

MIT
