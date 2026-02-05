# steamfetch

[![CI](https://github.com/unhappychoice/steamfetch/actions/workflows/ci.yml/badge.svg)](https://github.com/unhappychoice/steamfetch/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/steamfetch.svg)](https://crates.io/crates/steamfetch)
[![License: ISC](https://img.shields.io/badge/License-ISC-blue.svg)](https://opensource.org/licenses/ISC)

neofetch for Steam - Display your Steam stats in terminal with style.

## Screenshot

```
              .,,,,.                 
        .,'############',.           unhappychoice@Steam
     .'####################'.        ──────────────────────────────────────────────────
   .'#########################.      Member:       12 years        Eternal Witness
  ;###############'' ,.., '####,     Level:        74              Elite
 ;###############' .#;'':#, '###,    Games:        356             Dimension Hoarder
,###############,  #:    :#, ####,   Unplayed:     101 (28%)       I'll Play Tomorrow
'##############,   #.    ,#' #####   Playtime:     15,932h         Hermit of Eternity
  '*#########*'     '*,,*' .######   Perfect:      257             Platinum Overlord
     `'*###*'          ,.,;#######   Achievements: 38,580 (86%)    Chaos Incarnate
          .,;;,      .;############  
,',.         ';  ,###############'   Top Played
 '####. :,. .,; ,###############'    ├─ Idle Sphere          3,261h
  '####.. `'' .,###############'     ├─ Idle Cave Miner      1,162h
    '########################'       ├─ Revolution Idle      984h
      '*##################*'         ├─ PlanetSide 2         764h
         ''*##########*''            └─ Left 4 Dead 2        665h
              ''''''                 
                                     Recently Played (2 weeks)
                                     ├─ Elden Ring           20h 0m
                                     └─ Hades II             8h 0m
                                     
                                     Rarest: "One-stroke" (0.1%)
                                       in Q  REMASTERED
```

## Installation

### Install Script (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/unhappychoice/steamfetch/main/install.sh | bash
```

### From crates.io

```bash
cargo install steamfetch
```

### Download Binary

Download the latest release from [GitHub Releases](https://github.com/unhappychoice/steamfetch/releases).

### Build from Source

```bash
git clone https://github.com/unhappychoice/steamfetch.git
cd steamfetch
cargo build --release
./target/release/steamfetch
```

## Setup

### 1. Get Your Steam API Key

1. Visit https://steamcommunity.com/dev/apikey
2. Log in with your Steam account
3. Create a new API key

### 2. Find Your Steam ID

Visit https://steamid.io/ and enter your Steam profile URL.

### 3. Set Environment Variables

```bash
export STEAM_API_KEY="your_api_key_here"
export STEAM_ID="your_steam_id_here"
```

Add these to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.) for persistence.

## Usage

```bash
# Display your Steam stats
steamfetch

# Demo mode (no API key required)
steamfetch --demo

# Show version
steamfetch --version

# Show help
steamfetch --help
```

## Features

- Steam account stats (level, member since, games owned)
- Playtime statistics with fun titles
- Achievement progress and perfect games count
- Top played games list
- Recently played games (last 2 weeks)
- Rarest achievement display
- Beautiful SteamOS ASCII art with gradient colors
- Demo mode for testing without API setup

## How It Works

### With Steam Client Running

Uses Steamworks SDK for accurate game detection:
- Automatically detects logged-in Steam user
- Checks ownership for all known Steam games (~73,000 titles)
- Most accurate game count and achievement statistics

### Without Steam Client

Falls back to Steam Web API:
- Requires `STEAM_API_KEY` and `STEAM_ID` environment variables
- Returns games visible in your library
- Some owned games may not appear in API response

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

ISC License - see [LICENSE](LICENSE) for details.
