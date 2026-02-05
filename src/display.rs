use colored::Colorize;

use crate::steam::SteamStats;

pub fn render(stats: &SteamStats) {
    let info_lines = build_info_lines(stats);
    let logo_lines = build_logo();

    println!();
    for (i, logo_line) in logo_lines.iter().enumerate() {
        // Offset info by 1 line to align vertically
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

fn build_info_lines(stats: &SteamStats) -> Vec<String> {
    let mut lines = vec![
        format!("{}@{}", stats.username.bold().cyan(), "Steam".bold().cyan()),
        "─".repeat(35),
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

    for (i, game) in stats.top_games.iter().enumerate() {
        let prefix = if i == stats.top_games.len() - 1 {
            "└─"
        } else {
            "├─"
        };
        let name = truncate(&game.name, 20);
        lines.push(format!(
            "{} {} {}h",
            prefix,
            name,
            format_number(game.playtime_hours())
        ));
    }

    if let Some(ref achievements) = stats.achievement_stats {
        if let Some(ref rarest) = achievements.rarest {
            lines.push(String::new());
            lines.push(format!(
                "{}: \"{}\" ({:.1}%)",
                "Rarest".bold().yellow(),
                truncate(&rarest.name, 25).trim(),
                rarest.percent
            ));
            lines.push(format!(
                "  in {}",
                truncate(&rarest.game, 30).trim().dimmed()
            ));
        }
    }

    lines
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
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_len {
        format!("{:<width$}", s, width = max_len)
    } else {
        let truncated: String = chars[..max_len - 3].iter().collect();
        format!("{}...", truncated)
    }
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
