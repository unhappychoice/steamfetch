# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.7] - 2026-02-05

### Features

- feat: add verbose output for Web API debugging (#15) ([772d901](https://github.com/unhappychoice/steamfetch/commit/772d901))

### Other Changes

- chore: bump version to v0.2.7 ([e97e4ab](https://github.com/unhappychoice/steamfetch/commit/e97e4ab))


## [0.2.6] - 2026-02-05

### Bug Fixes

- fix: set DLL directory on Windows for steamclient64.dll dependencies (#14) ([07cefe0](https://github.com/unhappychoice/steamfetch/commit/07cefe0))

### Other Changes

- chore: bump version to v0.2.6 ([eeb2740](https://github.com/unhappychoice/steamfetch/commit/eeb2740))


## [0.2.5] - 2026-02-05

### Features

- feat: add --verbose flag for debugging Native SDK issues ([40d8f90](https://github.com/unhappychoice/steamfetch/commit/40d8f90))

### Bug Fixes

- fix: improve Windows registry parsing and add debug output ([0f55bac](https://github.com/unhappychoice/steamfetch/commit/0f55bac))

### Other Changes

- chore: bump version to v0.2.5 ([3363173](https://github.com/unhappychoice/steamfetch/commit/3363173))


## [0.2.4] - 2026-02-05

### Features

- feat: add Windows and macOS support for Native Steam SDK ([e78ff26](https://github.com/unhappychoice/steamfetch/commit/e78ff26))
- feat: detect WSL and install Windows binary to Windows path ([4639a87](https://github.com/unhappychoice/steamfetch/commit/4639a87))
- feat: add Windows support to install script ([d373ec4](https://github.com/unhappychoice/steamfetch/commit/d373ec4))

### Other Changes

- chore: bump version to v0.2.4 ([33f0ded](https://github.com/unhappychoice/steamfetch/commit/33f0ded))
- docs: change license from MIT to ISC ([3d754da](https://github.com/unhappychoice/steamfetch/commit/3d754da))
- docs: improve README with installation, usage, and contributing ([5d888e5](https://github.com/unhappychoice/steamfetch/commit/5d888e5))


## [0.2.3] - 2026-02-05

### Bug Fixes

- fix: search Steam API library from target directory ([113f2f7](https://github.com/unhappychoice/steamfetch/commit/113f2f7))

### Other Changes

- chore: bump version to v0.2.3 ([24e5f20](https://github.com/unhappychoice/steamfetch/commit/24e5f20))


## [0.2.2] - 2026-02-05

### Features

- feat: bundle Steam API library with release binaries ([2df0cde](https://github.com/unhappychoice/steamfetch/commit/2df0cde))

### Other Changes

- chore: bump version to v0.2.2 ([b52ccb6](https://github.com/unhappychoice/steamfetch/commit/b52ccb6))


## [0.2.1] - 2026-02-05

### Bug Fixes

- fix: use autostash for git pull in release workflow ([fbb3c39](https://github.com/unhappychoice/steamfetch/commit/fbb3c39))
- fix: stage changes before git pull in release workflow ([8612b18](https://github.com/unhappychoice/steamfetch/commit/8612b18))
- fix: pull latest before committing in release workflow ([202de07](https://github.com/unhappychoice/steamfetch/commit/202de07))

### Other Changes

- chore: bump version to v0.2.1 ([b8a5d04](https://github.com/unhappychoice/steamfetch/commit/b8a5d04))
- ci: remove ARM64 Linux from release targets (Steam unsupported) ([75165f1](https://github.com/unhappychoice/steamfetch/commit/75165f1))


## [0.2.0] - 2026-02-05

### Features

- feat: add install script ([9865868](https://github.com/unhappychoice/steamfetch/commit/9865868))
- feat: add recently played games and adjust line width ([e1f5e4d](https://github.com/unhappychoice/steamfetch/commit/e1f5e4d))
- feat: improve error messages with setup instructions ([7a4eb9b](https://github.com/unhappychoice/steamfetch/commit/7a4eb9b))
- feat(display): add title system with gradient colors ([e866773](https://github.com/unhappychoice/steamfetch/commit/e866773))
- feat(steam): add account_created, country, and steam_level fields ([994a211](https://github.com/unhappychoice/steamfetch/commit/994a211))
- feat: add SDK to Web API fallback ([c3a34e0](https://github.com/unhappychoice/steamfetch/commit/c3a34e0))
- feat(steam): add fetch_stats_for_appids method ([a4c2c12](https://github.com/unhappychoice/steamfetch/commit/a4c2c12))
- feat(steam): add native SDK client via steamclient.so ([b0511ba](https://github.com/unhappychoice/steamfetch/commit/b0511ba))
- feat(cli): add --demo flag for testing without API ([b7e8941](https://github.com/unhappychoice/steamfetch/commit/b7e8941))
- feat(steam): scan all games for accurate achievement stats ([9329bbd](https://github.com/unhappychoice/steamfetch/commit/9329bbd))
- feat(display): replace logo with SteamOS ASCII art ([23887cc](https://github.com/unhappychoice/steamfetch/commit/23887cc))
- feat: implement core Steam stats display with achievements ([8e12ff5](https://github.com/unhappychoice/steamfetch/commit/8e12ff5))
- feat: initial commit with README and cargo project ([e617dcd](https://github.com/unhappychoice/steamfetch/commit/e617dcd))

### Bug Fixes

- fix: remove unused fields and fix clippy warnings ([48f9f4f](https://github.com/unhappychoice/steamfetch/commit/48f9f4f))

### Other Changes

- chore: bump version to v0.2.0 ([a504f2a](https://github.com/unhappychoice/steamfetch/commit/a504f2a))
- ci: add release and CI workflows ([f1f81da](https://github.com/unhappychoice/steamfetch/commit/f1f81da))
- chore: update demo stats with new fields ([da76d9f](https://github.com/unhappychoice/steamfetch/commit/da76d9f))
- docs: update README with SDK usage and features ([c23437d](https://github.com/unhappychoice/steamfetch/commit/c23437d))
- style(display): simplify logo with single color #1b4b67 ([fedda0e](https://github.com/unhappychoice/steamfetch/commit/fedda0e))
- docs: update README with current features and API limitations ([0cf3de7](https://github.com/unhappychoice/steamfetch/commit/0cf3de7))


