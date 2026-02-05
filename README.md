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
# Set your Steam API key and Steam ID
export STEAM_API_KEY="your_api_key"
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
- [ ] Custom color themes
- [ ] JSON output

## Limitations

The Steam API only returns games currently in your library. Games you've played but no longer own (e.g., expired free-to-play licenses) are not included in the statistics. This means achievement counts may differ from services like completionist.me that track historical data.

## License

MIT
