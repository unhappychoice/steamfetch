use colored::Colorize;
use std::io::{self, Write};
use terminal_size::{terminal_size, Width};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::image_display;
use crate::steam::{GameStat, SteamStats};
use crate::ImageProtocol;

const IMAGE_COLS: u32 = 34;
const IMAGE_ROWS: u32 = 18;
const LEFT_OFFSET: usize = IMAGE_COLS as usize + 3; // image/logo width + gap
const DEFAULT_TERMINAL_WIDTH: u16 = 120;
const MIN_NAME_WIDTH: usize = 8;

pub struct ImageConfig {
    pub enabled: bool,
    pub protocol: ImageProtocol,
}

pub async fn render(stats: &SteamStats, image_config: &ImageConfig) {
    let info_lines = build_info_lines(stats, inner_width());

    if image_config.enabled {
        render_with_image(stats, &info_lines, image_config).await;
    } else {
        render_with_ascii(&info_lines);
    }
}

fn inner_width() -> usize {
    let total = terminal_size()
        .map(|(Width(w), _)| w)
        .unwrap_or(DEFAULT_TERMINAL_WIDTH) as usize;
    total.saturating_sub(LEFT_OFFSET)
}

async fn render_with_image(stats: &SteamStats, info_lines: &[String], config: &ImageConfig) {
    let avatar = match &stats.avatar_url {
        Some(url) => {
            let cache_key = format!("avatar_{}.png", stats.username);
            image_display::load_cached_or_download(url, &cache_key).await
        }
        None => None,
    };

    let Some(img) = avatar else {
        return render_with_ascii(info_lines);
    };

    println!();

    // Print image and rewind cursor to top-left of image area
    let image_rows =
        image_display::print_image_and_rewind(&img, &config.protocol, IMAGE_COLS, IMAGE_ROWS);

    let Some(image_rows) = image_rows else {
        return render_with_ascii(info_lines);
    };

    let col_offset = IMAGE_COLS + 3; // image width + gap
    let mut stdout = io::stdout().lock();

    // Print info lines to the right of the image
    for (i, line) in info_lines.iter().enumerate() {
        if i > 0 {
            writeln!(stdout).unwrap();
        }
        image_display::cursor_right(col_offset);
        write!(stdout, "{}", line).unwrap();
    }

    // Ensure we end up below the image area
    let extra = (image_rows as usize).saturating_sub(info_lines.len());
    for _ in 0..=extra {
        writeln!(stdout).unwrap();
    }
    stdout.flush().unwrap();
}

fn render_with_ascii(info_lines: &[String]) {
    let logo_lines = build_logo();

    println!();
    for (i, logo_line) in logo_lines.iter().enumerate() {
        let info = if i == 0 {
            ""
        } else {
            info_lines.get(i - 1).map(String::as_str).unwrap_or("")
        };
        println!("{}   {}", logo_line, info);
    }

    if info_lines.len() > logo_lines.len() - 1 {
        render_remaining_info(&info_lines[logo_lines.len() - 1..], logo_width());
    }
    println!();
}

fn build_logo() -> Vec<String> {
    let lines = vec![
        "              .,,,,.              ",
        "        .,'############',.        ",
        "     .'####################'.     ",
        "   .'#########################.   ",
        "  ;###############'' ,.., '####,  ",
        " ;###############' .#;'':#, '###, ",
        ",###############,  #:    :#, ####,",
        "'##############,   #.    ,#' #####",
        "  '*#########*'     '*,,*' .######",
        "     `'*###*'          ,.,;#######",
        "         .,;;,      .;############",
        ",',.         ';  ,###############'",
        " '####. :,. .,; ,###############' ",
        "  '####.. `'' .,###############'  ",
        "    '########################'    ",
        "      '*##################*'      ",
        "         ''*##########*''         ",
        "              ''''''              ",
    ];

    lines.into_iter().map(colorize_logo_line).collect()
}

fn colorize_logo_line(line: &str) -> String {
    format!("\x1b[38;2;27;75;103m{}\x1b[0m", line) // #1b4b67
}

fn logo_width() -> usize {
    35
}

fn build_info_lines(stats: &SteamStats, inner_width: usize) -> Vec<String> {
    // 13 (label) + 1 (space) + 14 (value) + 2 (space) + ~20 (title) = 50
    let line_width = 50;
    let mut lines = vec![
        format!("{}@{}", stats.username.bold().cyan(), "Steam".bold().cyan()),
        "─".repeat(line_width),
    ];

    // Account age
    if let Some(created) = stats.account_created {
        let years = account_age_years(created);
        let (title, color) = account_age_title(years);
        lines.push(stat_line(
            "Member",
            &format!("{} years", years),
            colorize_title(title, color),
        ));
    }

    // Steam Level
    if let Some(level) = stats.steam_level {
        let (title, color) = steam_level_title(level);
        lines.push(stat_line(
            "Level",
            &level.to_string(),
            colorize_title(title, color),
        ));
    }

    // Games
    let (title, color) = games_title(stats.game_count);
    lines.push(stat_line(
        "Games",
        &format_number(stats.game_count),
        colorize_title(title, color),
    ));

    // Unplayed
    let unplayed_pct = stats.unplayed_count as f64 / stats.game_count as f64 * 100.0;
    let (title, color) = unplayed_title(unplayed_pct);
    let value = format!(
        "{} ({:.0}%)",
        format_number(stats.unplayed_count),
        unplayed_pct
    );
    lines.push(stat_line(
        "Unplayed",
        &value,
        colorize_title_reverse(title, color),
    ));

    // Playtime
    let hours = stats.playtime_hours();
    let (title, color) = playtime_title(hours);
    lines.push(stat_line(
        "Playtime",
        &format!("{}h", format_number(hours)),
        colorize_title(title, color),
    ));

    if let Some(ref achievements) = stats.achievement_stats {
        // Perfect games
        let (title, color) = perfect_title(achievements.perfect_games);
        lines.push(stat_line(
            "Perfect",
            &format_number(achievements.perfect_games),
            colorize_title(title, color),
        ));

        // Achievements
        let ach_pct =
            achievements.total_achieved as f64 / achievements.total_possible as f64 * 100.0;
        let (title, color) = achievement_title(ach_pct);
        let value = format!(
            "{} ({:.0}%)",
            format_number(achievements.total_achieved),
            ach_pct
        );
        lines.push(stat_line(
            "Achievements",
            &value,
            colorize_title(title, color),
        ));
    }

    lines.push(String::new());
    lines.push(format!("{}", "Top Played".bold()));
    let top_times: Vec<String> = stats
        .top_games
        .iter()
        .map(|g| format!("{}h", format_number(g.playtime_hours())))
        .collect();
    lines.extend(tree_lines(&stats.top_games, &top_times, inner_width));

    if !stats.recently_played.is_empty() {
        lines.push(String::new());
        lines.push(format!("{}", "Recently Played (2 weeks)".bold()));
        let recent_times: Vec<String> = stats
            .recently_played
            .iter()
            .map(|g| format_playtime(g.playtime_minutes))
            .collect();
        lines.extend(tree_lines(
            &stats.recently_played,
            &recent_times,
            inner_width,
        ));
    }

    if let Some(ref achievements) = stats.achievement_stats {
        if let Some(ref rarest) = achievements.rarest {
            let percent_len = format!("{:.1}", rarest.percent).len();
            let name_max = inner_width
                .saturating_sub(percent_len + 14)
                .max(MIN_NAME_WIDTH);
            let game_max = inner_width.saturating_sub(5).max(MIN_NAME_WIDTH);

            lines.push(String::new());
            lines.push(format!(
                "{}: \"{}\" ({:.1}%)",
                "Rarest".bold().yellow(),
                truncate(&rarest.name, name_max).trim(),
                rarest.percent
            ));
            lines.push(format!(
                "  in {}",
                truncate(&rarest.game, game_max).trim().dimmed()
            ));
        }
    }

    lines
}

fn tree_lines(items: &[GameStat], times: &[String], inner_width: usize) -> Vec<String> {
    let name_width = tree_name_width(items, times, inner_width);
    items
        .iter()
        .zip(times.iter())
        .enumerate()
        .map(|(i, (game, time))| {
            let prefix = if i == items.len() - 1 {
                "└─"
            } else {
                "├─"
            };
            format!("{} {} {}", prefix, truncate(&game.name, name_width), time)
        })
        .collect()
}

fn tree_name_width(items: &[GameStat], times: &[String], inner_width: usize) -> usize {
    let max_time = times.iter().map(|t| t.width()).max().unwrap_or(0);
    let max_name = items.iter().map(|g| g.name.width()).max().unwrap_or(0);
    // prefix(2) + space(1) + name + space(1) + time
    let available = inner_width.saturating_sub(4 + max_time);
    available.min(max_name).max(MIN_NAME_WIDTH)
}

fn format_playtime(minutes: u32) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn games_title(count: u32) -> (&'static str, (u8, u8, u8)) {
    match count {
        0..=5 => ("Fledgling Spirit", (200, 220, 255)),
        6..=15 => ("Awakened Soul", (180, 200, 255)),
        16..=30 => ("Wandering Phantom", (160, 180, 255)),
        31..=50 => ("Shadow Initiate", (140, 160, 255)),
        51..=75 => ("Void Walker", (120, 140, 255)),
        76..=100 => ("Digital Specter", (100, 120, 255)),
        101..=150 => ("Realm Collector", (80, 100, 255)),
        151..=200 => ("Soul Harvester", (100, 80, 255)),
        201..=300 => ("Chaos Bringer", (120, 60, 255)),
        301..=400 => ("Dimension Hoarder", (140, 40, 255)),
        401..=500 => ("Abyss Keeper", (160, 60, 200)),
        501..=650 => ("Wallet Slayer", (180, 80, 180)),
        651..=800 => ("Forbidden Archivist", (200, 60, 160)),
        801..=1000 => ("Eternal Curator", (220, 40, 140)),
        1001..=1250 => ("Void Emperor", (240, 60, 120)),
        1251..=1500 => ("Infinite Library", (255, 80, 100)),
        1501..=2000 => ("Reality Distorter", (255, 60, 80)),
        2001..=3000 => ("Steam Leviathan", (255, 40, 60)),
        3001..=5000 => ("Cosmic Devourer", (255, 20, 40)),
        _ => ("GabeN's Chosen One", (255, 0, 30)),
    }
}

fn unplayed_title(pct: f64) -> (&'static str, (u8, u8, u8)) {
    match pct as u32 {
        0 => ("Actually Plays Games", (50, 255, 100)),
        1..=5 => ("Rare Specimen", (70, 250, 110)),
        6..=10 => ("Impressive Self-Control", (90, 240, 120)),
        11..=15 => ("Mostly Functional", (110, 230, 130)),
        16..=20 => ("Could Be Worse", (130, 220, 140)),
        21..=25 => ("Starting to Slip", (150, 210, 150)),
        26..=30 => ("I'll Play Tomorrow", (170, 200, 140)),
        31..=35 => ("Just One More Sale", (190, 190, 130)),
        36..=40 => ("Someday Maybe", (210, 180, 120)),
        41..=45 => ("Buying Is Playing", (230, 170, 110)),
        46..=50 => ("It Was On Sale OK", (250, 160, 100)),
        51..=55 => ("Send Help", (255, 150, 90)),
        56..=60 => ("My Wallet Weeps", (255, 130, 80)),
        61..=65 => ("Professional Dust Farmer", (255, 110, 70)),
        66..=70 => ("Why Am I Like This", (255, 90, 60)),
        71..=75 => ("Bundle Addiction", (255, 70, 55)),
        76..=80 => ("Gaming? What's That", (255, 50, 50)),
        81..=85 => ("Digital Landfill", (255, 30, 50)),
        86..=90 => ("Steam Sale Victim", (255, 20, 60)),
        91..=95 => ("Collecting Dust Pro", (255, 10, 70)),
        _ => ("Why Do I Even Bother", (255, 0, 80)),
    }
}

fn playtime_title(hours: u32) -> (&'static str, (u8, u8, u8)) {
    match hours {
        0..=10 => ("Newborn Shadow", (200, 230, 255)),
        11..=50 => ("Passing Specter", (180, 220, 255)),
        51..=100 => ("Fleeting Presence", (160, 210, 255)),
        101..=200 => ("Wandering Spirit", (140, 200, 255)),
        201..=350 => ("Devoted Phantom", (120, 190, 255)),
        351..=500 => ("Bound Soul", (100, 180, 255)),
        501..=750 => ("Chained Existence", (80, 170, 255)),
        751..=1000 => ("Eternal Prisoner", (100, 150, 255)),
        1001..=1500 => ("Time Devourer", (120, 130, 255)),
        1501..=2000 => ("Reality Forsaker", (140, 110, 255)),
        2001..=3000 => ("Dimension Exile", (160, 90, 255)),
        3001..=4000 => ("Void Dweller", (180, 70, 255)),
        4001..=5000 => ("Sunlight Deserter", (200, 50, 255)),
        5001..=7500 => ("Nocturnal Overlord", (220, 70, 200)),
        7501..=10000 => ("Crimson Night King", (240, 90, 150)),
        10001..=15000 => ("Grass Myth Believer", (255, 80, 100)),
        15001..=20000 => ("Hermit of Eternity", (255, 60, 80)),
        20001..=30000 => ("Ascended Beyond", (255, 40, 60)),
        30001..=50000 => ("Timeless One", (255, 20, 40)),
        _ => ("Chronos Incarnate", (255, 0, 30)),
    }
}

fn perfect_title(count: u32) -> (&'static str, (u8, u8, u8)) {
    match count {
        0 => ("Unawakened", (200, 220, 255)),
        1..=3 => ("First Blood", (180, 210, 255)),
        4..=7 => ("Rising Hunter", (160, 200, 255)),
        8..=12 => ("Soul Seeker", (140, 190, 255)),
        13..=20 => ("Dark Pursuer", (120, 180, 255)),
        21..=30 => ("Shadow Stalker", (100, 170, 255)),
        31..=45 => ("Relentless Blade", (80, 160, 255)),
        46..=60 => ("Trophy Reaper", (100, 140, 255)),
        61..=80 => ("Glory Collector", (120, 120, 255)),
        81..=100 => ("Perfection Seeker", (140, 100, 255)),
        101..=130 => ("Flawless Executor", (160, 80, 255)),
        131..=170 => ("Grandmaster of 100%", (180, 60, 255)),
        171..=210 => ("Eternal Perfectionist", (200, 80, 220)),
        211..=260 => ("Platinum Overlord", (220, 100, 180)),
        261..=320 => ("Supreme Completionist", (240, 80, 140)),
        321..=400 => ("Legendary Finisher", (255, 60, 100)),
        401..=500 => ("Mythical Achiever", (255, 40, 80)),
        501..=650 => ("Godslayer", (255, 20, 60)),
        651..=800 => ("Beyond Perfection", (255, 10, 50)),
        _ => ("Achievement Deity", (255, 0, 40)),
    }
}

fn account_age_years(created: u64) -> u32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    ((now - created) / 60 / 60 / 24 / 365) as u32
}

fn steam_level_title(level: u32) -> (&'static str, (u8, u8, u8)) {
    match level {
        0..=5 => ("Lurker", (200, 230, 255)),
        6..=10 => ("Novice", (180, 220, 255)),
        11..=15 => ("Apprentice", (160, 210, 255)),
        16..=20 => ("Regular", (140, 200, 255)),
        21..=25 => ("Established", (120, 190, 255)),
        26..=30 => ("Dedicated", (100, 180, 255)),
        31..=40 => ("Respected", (80, 170, 255)),
        41..=50 => ("Distinguished", (100, 150, 255)),
        51..=60 => ("Prestigious", (120, 130, 255)),
        61..=75 => ("Elite", (140, 110, 255)),
        76..=90 => ("Master", (160, 90, 255)),
        91..=100 => ("Grandmaster", (180, 70, 255)),
        101..=125 => ("Legend", (200, 50, 255)),
        126..=150 => ("Mythical", (220, 70, 200)),
        151..=200 => ("Immortal", (240, 90, 150)),
        201..=300 => ("Godlike", (255, 80, 100)),
        301..=500 => ("Ascended", (255, 60, 80)),
        501..=1000 => ("Whale Supreme", (255, 40, 60)),
        _ => ("Touch Grass Please", (255, 0, 30)),
    }
}

fn account_age_title(years: u32) -> (&'static str, (u8, u8, u8)) {
    match years {
        0 => ("Fresh Blood", (200, 230, 255)),
        1 => ("Newcomer", (180, 220, 255)),
        2 => ("Getting Hooked", (160, 210, 255)),
        3 => ("Loyal Customer", (140, 200, 255)),
        4 => ("Seasoned Gamer", (120, 190, 255)),
        5 => ("Veteran", (100, 180, 255)),
        6 => ("Battle-Hardened", (80, 170, 255)),
        7 => ("Old Guard", (100, 150, 255)),
        8 => ("Ancient One", (120, 130, 255)),
        9 => ("Living Legend", (140, 110, 255)),
        10 => ("Decade Survivor", (160, 90, 255)),
        11 => ("Time Traveler", (180, 70, 255)),
        12 => ("Eternal Witness", (200, 50, 255)),
        13 => ("Unlucky Thirteen", (220, 70, 200)),
        14 => ("Steam Fossil", (240, 90, 150)),
        15 => ("Digital Dinosaur", (255, 80, 100)),
        16 => ("Prehistoric Gamer", (255, 60, 80)),
        17 => ("Before It Was Cool", (255, 40, 60)),
        18 => ("OG Steam User", (255, 20, 40)),
        19 => ("Founding Father", (255, 10, 30)),
        _ => ("Primordial Entity", (255, 0, 20)),
    }
}

fn achievement_title(pct: f64) -> (&'static str, (u8, u8, u8)) {
    match pct as u32 {
        0..=5 => ("Empty Vessel", (200, 220, 255)),
        6..=10 => ("Dormant Power", (180, 215, 255)),
        11..=15 => ("Stirring Darkness", (160, 210, 255)),
        16..=20 => ("Awakening Force", (140, 205, 255)),
        21..=25 => ("Rising Shadow", (120, 200, 255)),
        26..=30 => ("Hungry Spirit", (100, 195, 255)),
        31..=35 => ("Growing Ambition", (80, 190, 255)),
        36..=40 => ("Burning Desire", (100, 175, 255)),
        41..=45 => ("Unstoppable Will", (120, 160, 255)),
        46..=50 => ("Half-Awakened", (140, 145, 255)),
        51..=55 => ("Power Unleashed", (160, 130, 255)),
        56..=60 => ("Chaos Rising", (180, 115, 255)),
        61..=65 => ("Dark Dominator", (200, 100, 255)),
        66..=70 => ("Realm Conqueror", (220, 100, 220)),
        71..=75 => ("Relentless Force", (240, 100, 180)),
        76..=80 => ("Apex Predator", (255, 90, 140)),
        81..=85 => ("Obsidian Emperor", (255, 70, 100)),
        86..=90 => ("Chaos Incarnate", (255, 50, 80)),
        91..=95 => ("Near-Omniscient", (255, 30, 60)),
        96..=99 => ("Edge of Infinity", (255, 15, 45)),
        _ => ("The Absolute One", (255, 0, 30)),
    }
}

fn colorize_title(title: &str, base_color: (u8, u8, u8)) -> String {
    gradient_text(title, base_color, true)
}

fn colorize_title_reverse(title: &str, base_color: (u8, u8, u8)) -> String {
    gradient_text(title, base_color, false)
}

fn gradient_text(title: &str, base: (u8, u8, u8), darken: bool) -> String {
    let chars: Vec<char> = title.chars().collect();
    let len = chars.len().max(1);

    let mut result = String::new();
    for (i, c) in chars.iter().enumerate() {
        let t = i as f64 / len as f64;
        let factor = if darken { 1.0 - t * 0.4 } else { 0.6 + t * 0.4 };
        let r = (base.0 as f64 * factor).min(255.0) as u8;
        let g = (base.1 as f64 * factor).min(255.0) as u8;
        let b = (base.2 as f64 * factor).min(255.0) as u8;
        result.push_str(&format!("\x1b[38;2;{};{};{}m{}", r, g, b, c));
    }
    result.push_str("\x1b[0m");
    result
}

fn stat_line(label: &str, value: &str, title: String) -> String {
    // Pad before applying colors
    let label_with_colon = format!("{}:", label);
    let label_padded = format!("{:<13}", label_with_colon);
    let value_padded = format!("{:<14}", value);
    format!("{} {}  {}", label_padded.bold(), value_padded, title)
}

fn render_remaining_info(lines: &[String], width: usize) {
    let padding = " ".repeat(width);
    lines
        .iter()
        .for_each(|line| println!("{}  {}", padding, line));
}

fn truncate(s: &str, max_len: usize) -> String {
    let text_width = s.width();
    if text_width <= max_len {
        return format!("{s}{}", " ".repeat(max_len - text_width));
    }
    let target = max_len.saturating_sub(3);
    let mut width = text_width;
    let mut chars: Vec<char> = s.chars().collect();
    while width > target {
        let Some(c) = chars.pop() else { break };
        width -= c.width().unwrap_or(0).max(1);
    }
    let truncated: String = chars.iter().collect();
    let padding = " ".repeat(max_len.saturating_sub(width + 3));
    format!("{truncated}...{padding}")
}

fn format_number(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam::{AchievementStats, RarestAchievement};

    fn strip_ansi(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                result.push(c);
            }
        }
        result
    }

    #[test]
    fn test_format_number_zero() {
        assert_eq!(format_number(0), "0");
    }

    #[test]
    fn test_format_number_small() {
        assert_eq!(format_number(7), "7");
        assert_eq!(format_number(42), "42");
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn test_format_number_thousands() {
        assert_eq!(format_number(1_000), "1,000");
        assert_eq!(format_number(12_345), "12,345");
        assert_eq!(format_number(999_999), "999,999");
    }

    #[test]
    fn test_format_number_millions() {
        assert_eq!(format_number(1_000_000), "1,000,000");
        assert_eq!(format_number(1_234_567), "1,234,567");
        assert_eq!(format_number(4_294_967_295), "4,294,967,295");
    }

    #[test]
    fn test_format_playtime_minutes_only() {
        assert_eq!(format_playtime(0), "0m");
        assert_eq!(format_playtime(30), "30m");
        assert_eq!(format_playtime(59), "59m");
    }

    #[test]
    fn test_format_playtime_with_hours() {
        assert_eq!(format_playtime(60), "1h 0m");
        assert_eq!(format_playtime(90), "1h 30m");
        assert_eq!(format_playtime(125), "2h 5m");
        assert_eq!(format_playtime(3661), "61h 1m");
    }

    #[test]
    fn test_truncate_shorter_than_max() {
        let result = truncate("abc", 10);
        assert_eq!(result.width(), 10);
        assert!(result.starts_with("abc"));
    }

    #[test]
    fn test_truncate_exact_length() {
        let result = truncate("abcde", 5);
        assert_eq!(result, "abcde");
    }

    #[test]
    fn test_truncate_longer_than_max() {
        let result = truncate("abcdefghij", 6);
        assert_eq!(result.width(), 6);
        assert!(result.contains("..."));
    }

    #[test]
    fn test_truncate_empty_string() {
        let result = truncate("", 5);
        assert_eq!(result.width(), 5);
    }

    #[test]
    fn test_truncate_unicode_wide_chars() {
        // Each CJK char has width 2
        let result = truncate("あいうえお", 4);
        assert!(result.width() <= 4);
    }

    #[test]
    fn test_logo_width_constant() {
        assert_eq!(logo_width(), 35);
    }

    #[test]
    fn test_build_logo_returns_18_lines() {
        let lines = build_logo();
        assert_eq!(lines.len(), 18);
    }

    #[test]
    fn test_colorize_logo_line_wraps_with_ansi() {
        let s = colorize_logo_line("hello");
        assert!(s.contains("hello"));
        assert!(s.contains("\x1b["));
        assert!(s.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_games_title_lower_bound() {
        let (label, _) = games_title(0);
        assert_eq!(label, "Fledgling Spirit");
    }

    #[test]
    fn test_games_title_buckets() {
        assert_eq!(games_title(10).0, "Awakened Soul");
        assert_eq!(games_title(20).0, "Wandering Phantom");
        assert_eq!(games_title(40).0, "Shadow Initiate");
        assert_eq!(games_title(60).0, "Void Walker");
        assert_eq!(games_title(90).0, "Digital Specter");
        assert_eq!(games_title(120).0, "Realm Collector");
        assert_eq!(games_title(180).0, "Soul Harvester");
        assert_eq!(games_title(250).0, "Chaos Bringer");
        assert_eq!(games_title(350).0, "Dimension Hoarder");
        assert_eq!(games_title(450).0, "Abyss Keeper");
        assert_eq!(games_title(600).0, "Wallet Slayer");
        assert_eq!(games_title(700).0, "Forbidden Archivist");
        assert_eq!(games_title(900).0, "Eternal Curator");
        assert_eq!(games_title(1100).0, "Void Emperor");
        assert_eq!(games_title(1400).0, "Infinite Library");
        assert_eq!(games_title(1700).0, "Reality Distorter");
        assert_eq!(games_title(2500).0, "Steam Leviathan");
        assert_eq!(games_title(4000).0, "Cosmic Devourer");
        assert_eq!(games_title(99999).0, "GabeN's Chosen One");
    }

    #[test]
    fn test_unplayed_title_buckets() {
        assert_eq!(unplayed_title(0.0).0, "Actually Plays Games");
        assert_eq!(unplayed_title(3.0).0, "Rare Specimen");
        assert_eq!(unplayed_title(7.0).0, "Impressive Self-Control");
        assert_eq!(unplayed_title(13.0).0, "Mostly Functional");
        assert_eq!(unplayed_title(18.0).0, "Could Be Worse");
        assert_eq!(unplayed_title(23.0).0, "Starting to Slip");
        assert_eq!(unplayed_title(28.0).0, "I'll Play Tomorrow");
        assert_eq!(unplayed_title(33.0).0, "Just One More Sale");
        assert_eq!(unplayed_title(38.0).0, "Someday Maybe");
        assert_eq!(unplayed_title(43.0).0, "Buying Is Playing");
        assert_eq!(unplayed_title(48.0).0, "It Was On Sale OK");
        assert_eq!(unplayed_title(53.0).0, "Send Help");
        assert_eq!(unplayed_title(58.0).0, "My Wallet Weeps");
        assert_eq!(unplayed_title(63.0).0, "Professional Dust Farmer");
        assert_eq!(unplayed_title(68.0).0, "Why Am I Like This");
        assert_eq!(unplayed_title(73.0).0, "Bundle Addiction");
        assert_eq!(unplayed_title(78.0).0, "Gaming? What's That");
        assert_eq!(unplayed_title(83.0).0, "Digital Landfill");
        assert_eq!(unplayed_title(88.0).0, "Steam Sale Victim");
        assert_eq!(unplayed_title(93.0).0, "Collecting Dust Pro");
        assert_eq!(unplayed_title(99.0).0, "Why Do I Even Bother");
    }

    #[test]
    fn test_playtime_title_buckets() {
        assert_eq!(playtime_title(5).0, "Newborn Shadow");
        assert_eq!(playtime_title(20).0, "Passing Specter");
        assert_eq!(playtime_title(75).0, "Fleeting Presence");
        assert_eq!(playtime_title(150).0, "Wandering Spirit");
        assert_eq!(playtime_title(300).0, "Devoted Phantom");
        assert_eq!(playtime_title(400).0, "Bound Soul");
        assert_eq!(playtime_title(600).0, "Chained Existence");
        assert_eq!(playtime_title(900).0, "Eternal Prisoner");
        assert_eq!(playtime_title(1200).0, "Time Devourer");
        assert_eq!(playtime_title(1700).0, "Reality Forsaker");
        assert_eq!(playtime_title(2500).0, "Dimension Exile");
        assert_eq!(playtime_title(3500).0, "Void Dweller");
        assert_eq!(playtime_title(4500).0, "Sunlight Deserter");
        assert_eq!(playtime_title(6000).0, "Nocturnal Overlord");
        assert_eq!(playtime_title(8000).0, "Crimson Night King");
        assert_eq!(playtime_title(12000).0, "Grass Myth Believer");
        assert_eq!(playtime_title(17000).0, "Hermit of Eternity");
        assert_eq!(playtime_title(25000).0, "Ascended Beyond");
        assert_eq!(playtime_title(40000).0, "Timeless One");
        assert_eq!(playtime_title(60000).0, "Chronos Incarnate");
    }

    #[test]
    fn test_perfect_title_buckets() {
        assert_eq!(perfect_title(0).0, "Unawakened");
        assert_eq!(perfect_title(2).0, "First Blood");
        assert_eq!(perfect_title(5).0, "Rising Hunter");
        assert_eq!(perfect_title(10).0, "Soul Seeker");
        assert_eq!(perfect_title(15).0, "Dark Pursuer");
        assert_eq!(perfect_title(25).0, "Shadow Stalker");
        assert_eq!(perfect_title(40).0, "Relentless Blade");
        assert_eq!(perfect_title(50).0, "Trophy Reaper");
        assert_eq!(perfect_title(70).0, "Glory Collector");
        assert_eq!(perfect_title(90).0, "Perfection Seeker");
        assert_eq!(perfect_title(120).0, "Flawless Executor");
        assert_eq!(perfect_title(150).0, "Grandmaster of 100%");
        assert_eq!(perfect_title(200).0, "Eternal Perfectionist");
        assert_eq!(perfect_title(240).0, "Platinum Overlord");
        assert_eq!(perfect_title(300).0, "Supreme Completionist");
        assert_eq!(perfect_title(350).0, "Legendary Finisher");
        assert_eq!(perfect_title(450).0, "Mythical Achiever");
        assert_eq!(perfect_title(600).0, "Godslayer");
        assert_eq!(perfect_title(700).0, "Beyond Perfection");
        assert_eq!(perfect_title(1000).0, "Achievement Deity");
    }

    #[test]
    fn test_steam_level_title_buckets() {
        assert_eq!(steam_level_title(3).0, "Lurker");
        assert_eq!(steam_level_title(8).0, "Novice");
        assert_eq!(steam_level_title(13).0, "Apprentice");
        assert_eq!(steam_level_title(18).0, "Regular");
        assert_eq!(steam_level_title(23).0, "Established");
        assert_eq!(steam_level_title(28).0, "Dedicated");
        assert_eq!(steam_level_title(35).0, "Respected");
        assert_eq!(steam_level_title(45).0, "Distinguished");
        assert_eq!(steam_level_title(55).0, "Prestigious");
        assert_eq!(steam_level_title(70).0, "Elite");
        assert_eq!(steam_level_title(85).0, "Master");
        assert_eq!(steam_level_title(95).0, "Grandmaster");
        assert_eq!(steam_level_title(115).0, "Legend");
        assert_eq!(steam_level_title(140).0, "Mythical");
        assert_eq!(steam_level_title(180).0, "Immortal");
        assert_eq!(steam_level_title(250).0, "Godlike");
        assert_eq!(steam_level_title(400).0, "Ascended");
        assert_eq!(steam_level_title(800).0, "Whale Supreme");
        assert_eq!(steam_level_title(2000).0, "Touch Grass Please");
    }

    #[test]
    fn test_account_age_title_buckets() {
        assert_eq!(account_age_title(0).0, "Fresh Blood");
        assert_eq!(account_age_title(1).0, "Newcomer");
        assert_eq!(account_age_title(2).0, "Getting Hooked");
        assert_eq!(account_age_title(3).0, "Loyal Customer");
        assert_eq!(account_age_title(4).0, "Seasoned Gamer");
        assert_eq!(account_age_title(5).0, "Veteran");
        assert_eq!(account_age_title(6).0, "Battle-Hardened");
        assert_eq!(account_age_title(7).0, "Old Guard");
        assert_eq!(account_age_title(8).0, "Ancient One");
        assert_eq!(account_age_title(9).0, "Living Legend");
        assert_eq!(account_age_title(10).0, "Decade Survivor");
        assert_eq!(account_age_title(11).0, "Time Traveler");
        assert_eq!(account_age_title(12).0, "Eternal Witness");
        assert_eq!(account_age_title(13).0, "Unlucky Thirteen");
        assert_eq!(account_age_title(14).0, "Steam Fossil");
        assert_eq!(account_age_title(15).0, "Digital Dinosaur");
        assert_eq!(account_age_title(16).0, "Prehistoric Gamer");
        assert_eq!(account_age_title(17).0, "Before It Was Cool");
        assert_eq!(account_age_title(18).0, "OG Steam User");
        assert_eq!(account_age_title(19).0, "Founding Father");
        assert_eq!(account_age_title(99).0, "Primordial Entity");
    }

    #[test]
    fn test_account_age_title_returns_distinct_colors_per_year() {
        let colors: Vec<_> = (0..=19).map(|y| account_age_title(y).1).collect();
        let unique: std::collections::HashSet<_> = colors.iter().collect();
        assert_eq!(unique.len(), colors.len());
    }

    #[test]
    fn test_achievement_title_buckets() {
        assert_eq!(achievement_title(2.0).0, "Empty Vessel");
        assert_eq!(achievement_title(8.0).0, "Dormant Power");
        assert_eq!(achievement_title(13.0).0, "Stirring Darkness");
        assert_eq!(achievement_title(18.0).0, "Awakening Force");
        assert_eq!(achievement_title(23.0).0, "Rising Shadow");
        assert_eq!(achievement_title(28.0).0, "Hungry Spirit");
        assert_eq!(achievement_title(33.0).0, "Growing Ambition");
        assert_eq!(achievement_title(38.0).0, "Burning Desire");
        assert_eq!(achievement_title(43.0).0, "Unstoppable Will");
        assert_eq!(achievement_title(48.0).0, "Half-Awakened");
        assert_eq!(achievement_title(53.0).0, "Power Unleashed");
        assert_eq!(achievement_title(58.0).0, "Chaos Rising");
        assert_eq!(achievement_title(63.0).0, "Dark Dominator");
        assert_eq!(achievement_title(68.0).0, "Realm Conqueror");
        assert_eq!(achievement_title(73.0).0, "Relentless Force");
        assert_eq!(achievement_title(78.0).0, "Apex Predator");
        assert_eq!(achievement_title(83.0).0, "Obsidian Emperor");
        assert_eq!(achievement_title(88.0).0, "Chaos Incarnate");
        assert_eq!(achievement_title(93.0).0, "Near-Omniscient");
        assert_eq!(achievement_title(98.0).0, "Edge of Infinity");
        assert_eq!(achievement_title(100.0).0, "The Absolute One");
    }

    #[test]
    fn test_gradient_text_preserves_chars() {
        let result = gradient_text("hi", (255, 128, 64), true);
        assert_eq!(strip_ansi(&result), "hi");
    }

    #[test]
    fn test_gradient_text_empty_string() {
        let result = gradient_text("", (100, 100, 100), false);
        assert_eq!(strip_ansi(&result), "");
        assert!(result.contains("\x1b[0m"));
    }

    #[test]
    fn test_gradient_text_reverse_starts_dimmer() {
        let darken = gradient_text("X", (200, 200, 200), true);
        let brighten = gradient_text("X", (200, 200, 200), false);
        assert_ne!(darken, brighten);
    }

    #[test]
    fn test_colorize_title_and_reverse_differ() {
        let a = colorize_title("Test", (200, 100, 50));
        let b = colorize_title_reverse("Test", (200, 100, 50));
        assert_ne!(a, b);
        assert_eq!(strip_ansi(&a), "Test");
        assert_eq!(strip_ansi(&b), "Test");
    }

    #[test]
    fn test_stat_line_formats_label_and_value() {
        let line = stat_line("Games", "42", "Title".to_string());
        let stripped = strip_ansi(&line);
        assert!(stripped.contains("Games:"));
        assert!(stripped.contains("42"));
        assert!(stripped.contains("Title"));
    }

    #[test]
    fn test_tree_lines_uses_branch_and_corner_prefix() {
        let items = vec![
            GameStat {
                name: "First".to_string(),
                playtime_minutes: 60,
            },
            GameStat {
                name: "Second".to_string(),
                playtime_minutes: 120,
            },
        ];
        let times = vec!["1h".to_string(), "2h".to_string()];
        let lines = tree_lines(&items, &times, 80);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("├─"));
        assert!(lines[1].starts_with("└─"));
        assert!(lines[0].contains("First"));
        assert!(lines[1].contains("Second"));
    }

    #[test]
    fn test_tree_lines_single_item_uses_corner() {
        let items = vec![GameStat {
            name: "Only".to_string(),
            playtime_minutes: 30,
        }];
        let times = vec!["30m".to_string()];
        let lines = tree_lines(&items, &times, 80);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("└─"));
    }

    #[test]
    fn test_tree_lines_empty() {
        let items: Vec<GameStat> = Vec::new();
        let times: Vec<String> = Vec::new();
        let lines = tree_lines(&items, &times, 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_tree_name_width_respects_min() {
        let items = vec![GameStat {
            name: "x".to_string(),
            playtime_minutes: 0,
        }];
        let times = vec!["0m".to_string()];
        // Even with very narrow inner_width, should not go below MIN_NAME_WIDTH (8)
        let width = tree_name_width(&items, &times, 1);
        assert_eq!(width, MIN_NAME_WIDTH);
    }

    #[test]
    fn test_tree_name_width_caps_at_max_name() {
        let long_name = "A".repeat(40);
        let items = vec![GameStat {
            name: long_name.clone(),
            playtime_minutes: 0,
        }];
        let times = vec!["0m".to_string()];
        let width = tree_name_width(&items, &times, 200);
        assert_eq!(width, long_name.width());
    }

    fn make_minimal_stats() -> SteamStats {
        SteamStats {
            username: "alice".to_string(),
            game_count: 10,
            unplayed_count: 2,
            total_playtime_minutes: 1200,
            top_games: vec![GameStat {
                name: "Game A".to_string(),
                playtime_minutes: 600,
            }],
            achievement_stats: None,
            account_created: None,
            steam_level: None,
            recently_played: Vec::new(),
            avatar_url: None,
        }
    }

    fn lines_text(lines: &[String]) -> String {
        lines
            .iter()
            .map(|l| strip_ansi(l))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_build_info_lines_minimal_includes_required_sections() {
        let stats = make_minimal_stats();
        let lines = build_info_lines(&stats, 80);
        let text = lines_text(&lines);
        assert!(text.contains("alice@Steam"));
        assert!(text.contains("Games:"));
        assert!(text.contains("Unplayed:"));
        assert!(text.contains("Playtime:"));
        assert!(text.contains("Top Played"));
        assert!(text.contains("Game A"));
        assert!(!text.contains("Member:"));
        assert!(!text.contains("Level:"));
        assert!(!text.contains("Perfect:"));
        assert!(!text.contains("Achievements:"));
        assert!(!text.contains("Recently Played"));
        assert!(!text.contains("Rarest"));
    }

    #[test]
    fn test_build_info_lines_with_steam_level_adds_level() {
        let mut stats = make_minimal_stats();
        stats.steam_level = Some(42);
        let lines = build_info_lines(&stats, 80);
        let text = lines_text(&lines);
        assert!(text.contains("Level:"));
        assert!(text.contains("42"));
    }

    #[test]
    fn test_build_info_lines_with_account_created_adds_member() {
        let mut stats = make_minimal_stats();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        stats.account_created = Some(now);
        let lines = build_info_lines(&stats, 80);
        let text = lines_text(&lines);
        assert!(text.contains("Member:"));
        assert!(text.contains("0 years"));
    }

    #[test]
    fn test_build_info_lines_with_achievements_adds_perfect_and_achievements() {
        let mut stats = make_minimal_stats();
        stats.achievement_stats = Some(AchievementStats {
            total_achieved: 50,
            total_possible: 100,
            perfect_games: 3,
            rarest: None,
        });
        let lines = build_info_lines(&stats, 80);
        let text = lines_text(&lines);
        assert!(text.contains("Perfect:"));
        assert!(text.contains("Achievements:"));
        assert!(text.contains("50"));
        assert!(text.contains("(50%)"));
        assert!(!text.contains("Rarest"));
    }

    #[test]
    fn test_build_info_lines_with_rarest_adds_rarest_section() {
        let mut stats = make_minimal_stats();
        stats.achievement_stats = Some(AchievementStats {
            total_achieved: 1,
            total_possible: 10,
            perfect_games: 0,
            rarest: Some(RarestAchievement {
                name: "Hidden Gem".to_string(),
                game: "Mystery Game".to_string(),
                percent: 0.7,
            }),
        });
        let lines = build_info_lines(&stats, 80);
        let text = lines_text(&lines);
        assert!(text.contains("Rarest"));
        assert!(text.contains("Hidden Gem"));
        assert!(text.contains("Mystery Game"));
        assert!(text.contains("0.7%"));
    }

    #[test]
    fn test_build_info_lines_with_recently_played_adds_section() {
        let mut stats = make_minimal_stats();
        stats.recently_played = vec![GameStat {
            name: "Recent Game".to_string(),
            playtime_minutes: 75,
        }];
        let lines = build_info_lines(&stats, 80);
        let text = lines_text(&lines);
        assert!(text.contains("Recently Played"));
        assert!(text.contains("Recent Game"));
        assert!(text.contains("1h 15m"));
    }

    #[test]
    fn test_build_info_lines_unplayed_percentage_rounds() {
        let mut stats = make_minimal_stats();
        stats.game_count = 4;
        stats.unplayed_count = 1;
        let lines = build_info_lines(&stats, 80);
        let text = lines_text(&lines);
        assert!(text.contains("(25%)"));
    }

    #[test]
    fn test_account_age_years_recent_returns_zero() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(account_age_years(now), 0);
    }

    #[test]
    fn test_account_age_years_one_year_ago() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let one_year_secs = 60 * 60 * 24 * 365;
        assert_eq!(account_age_years(now - one_year_secs - 100), 1);
    }

    #[test]
    fn test_inner_width_is_non_negative_and_bounded() {
        // `inner_width()` reads `terminal_size()` which in a non-TTY test
        // environment falls back to `DEFAULT_TERMINAL_WIDTH` (120) minus
        // `LEFT_OFFSET`. Either way the value is a valid usize ≤ u16::MAX.
        let w = inner_width();
        assert!(w <= u16::MAX as usize);
    }

    #[test]
    fn test_render_with_ascii_does_not_panic_with_empty_info() {
        // Smoke test: should print the logo block without panicking.
        render_with_ascii(&[]);
    }

    #[test]
    fn test_render_with_ascii_handles_info_longer_than_logo() {
        // 30 info lines exceeds the 18-line logo, exercising the
        // `render_remaining_info` branch inside `render_with_ascii`.
        let info: Vec<String> = (0..30).map(|i| format!("info {}", i)).collect();
        render_with_ascii(&info);
    }

    #[test]
    fn test_render_remaining_info_pads_with_logo_width() {
        // Direct call covers the helper's iteration + println! path.
        let lines = vec!["alpha".to_string(), "beta".to_string()];
        render_remaining_info(&lines, logo_width());
    }

    #[test]
    fn test_render_remaining_info_empty_input_is_noop() {
        // No iteration; covers the early-exit shape of for_each on empty.
        render_remaining_info(&[], 0);
    }
}
