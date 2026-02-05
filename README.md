# steamfetch

neofetch for Steam - Display your Steam stats in terminal.

## Screenshot

```
$ steamfetch

              .,,,,.                unhappychoice@Steam
        .'onNMMMMMNNnn',.           ─────────────────────
     .'oNMANKMMMMMMMMMMMNNn'.       Games: 486 (123 unplayed)
   .'ANMMMMMMMXKNNWWWPFFWNNMNn.     Playtime: 2,847h (118 days)
  ;NNMMMMMMMMMMNWW'' ,.., 'WMMM,    Perfect: 24 games
 ;NMMMMV+##+VNWWW' .+;'':+, 'WMW,   Achievements: 3,241 / 5,892 (55%)
,VNNWP+######+WW,  +:    :+, +MMM,
'+#############,   +.    ,+' +NMMM  Top Played
  '*#########*'     '*,,*' .+NMMMM  ├─ Borderlands 3      478h
     `'*###*'          ,.,;###+WNM  ├─ Coin Push RPG      377h
         .,;;,      .;##########+W  └─ DRG Survivor       252h
,',.         ';  ,+##############'
 '###+. :,. .,; ,###############'   Rarest: "Impossible Task" (0.1%)
  '####.. `'' .,###############'      in Dark Souls III
    '#####+++################'
      '*##################*'
         ''*##########*''
              ''''''
```

## Installation

```bash
cargo install steamfetch
```

## Usage

```bash
# Set your Steam API key
export STEAM_API_KEY="your_api_key"

# If Steam client is running: uses Steamworks SDK (accurate)
# If Steam client is not running: uses Web API (requires STEAM_ID)
export STEAM_ID="your_steam_id"

# Run
steamfetch

# Demo mode (no API key required)
steamfetch --demo
```

Get your API key at: https://steamcommunity.com/dev/apikey

Find your Steam ID at: https://steamid.io/

## Features

- [x] Display game count and playtime
- [x] Show achievement stats
- [x] Top played games
- [x] Rarest achievements
- [x] SteamOS ASCII art logo
- [x] Demo mode for testing
- [x] Steamworks SDK for accurate detection (auto-fallback to Web API)
- [ ] Custom color themes
- [ ] JSON output

## How It Works

### With Steam Client Running (Recommended)

Uses Steamworks SDK for accurate game detection:
- Checks ownership for all known Steam games (~73,000 titles)
- Accurate game count and achievement statistics

### Without Steam Client

Falls back to Steam Web API:
- Only returns games currently visible in your library
- Some owned games may not appear in the API response
- Requires `STEAM_ID` environment variable

## License

MIT
