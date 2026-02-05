# steamfetch

neofetch for Steam - Display your Steam stats in terminal.

## Screenshot

```
$ steamfetch

        ╱╲                unhappychoice@Steam
       ╱  ╲               ─────────────────────
      ╱    ╲              Games: 486 (123 unplayed)
     ╱  ▲▲  ╲             Playtime: 2,847h (118 days)
    ╱  ▲▲▲▲  ╲            Perfect: 24 games
   ╱__________╲           Achievements: 3,241 / 5,892 (55%)
   \__________/           
                          Top Played
                          ├─ Borderlands 3      478h
                          ├─ Coin Push RPG      377h
                          └─ DRG Survivor       252h

                          Rarest: "Impossible Task" (0.1%)
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
```

Get your API key at: https://steamcommunity.com/dev/apikey

## Features

- [x] Display game count and playtime
- [ ] Show achievement stats
- [ ] Top played games
- [ ] Rarest achievements
- [ ] Custom color themes
- [ ] ASCII art customization
- [ ] JSON output

## License

MIT
