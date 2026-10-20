# Headless CLI

`moviebox-cli` is the same provider engine as the terminal UI, without a terminal. It
is meant for scripts, containers and anything that would otherwise have to parse the
TUI.

```bash
moviebox-cli providers
moviebox-cli search "dune" --json
moviebox-cli details <subject-id> --json
moviebox-cli streams <subject-id> --season 1 --episode 2 --json
moviebox-cli play <subject-id> --season 1 --episode 2
```

The binary ships in the same release archives as `moviebox-tui`.

## Commands

| Command | What it does |
| :--- | :--- |
| `providers` | Lists the providers and what they support. |
| `search <query>` | Searches the catalog. |
| `details <subject-id>` | Metadata, seasons and episodes of a title. |
| `streams <subject-id>` | The playable sources of a title or episode. |
| `play <subject-id>` | Resolves a source and hands it to an external player. |

## Options

| Option | Meaning |
| :--- | :--- |
| `--provider <id>` | `moviebox` (default), `fourkhdhub`, `dramachi`, `bdix_circleftp`, `bdix_dhakaflix`. |
| `--page <n>` | Search page, default 1. |
| `--limit <n>` | Maximum number of results. |
| `--season <n>` / `--episode <n>` | Episode selection. |
| `--player <kind>` | `mpv`, `vlc`, `iina` or `android`. Auto-detected by default. |
| `--print-url` | Print the resolved URL and headers instead of playing. |
| `--json` | Machine readable output on stdout. |

## Scripting

With `--json` the output is a single JSON document, so it pipes straight into `jq`:

```bash
# the first five results of a search
moviebox-cli search "dune" --json | jq -r '.items[] | "\(.id.value)\t\(.title)"'

# the 1080p source of an episode, ready for curl
moviebox-cli streams 985706818477379368 --season 1 --episode 1 --json \
  | jq -r '.releases[] | select(.quality == "1080p") | .mirrors[0].resolver_url'

# download it with the headers the CDN needs
url=$(moviebox-cli play 1511967966181605720 --print-url --json | jq -r .url)
ua=$(moviebox-cli play 1511967966181605720 --print-url --json | jq -r '.headers[] | select(.[0] == "User-Agent") | .[1]')
curl -H "User-Agent: $ua" -o dune.mp4 "$url"
```

`play --print-url` resolves a **fresh** link, because the MovieBox CDN signs its URLs
and they expire.

## Notes

- The player is only used for `play`; the other commands never touch the terminal.
- The engine is the same one the terminal client and the Android app use, so a provider
  that is unavailable in the app is unavailable here too.
- Exit codes: `0` on success, `1` on a runtime error (a provider that is down, a
  title without sources), `2` on a usage error.
