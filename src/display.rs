use colored::Colorize;

use crate::steam::SteamStats;

pub fn render(stats: &SteamStats) {
    let info_lines = build_info_lines(stats);
    let logo_lines = build_logo();

    println!();
    for (i, logo_line) in logo_lines.iter().enumerate() {
        let info = info_lines.get(i).map(String::as_str).unwrap_or("");
        println!("{}  {}", logo_line, info);
    }

    if info_lines.len() > logo_lines.len() {
        render_remaining_info(&info_lines[logo_lines.len()..], logo_width());
    }
    println!();
}

fn build_logo() -> Vec<String> {
    let width = logo_width();
    let lines: Vec<(&str, bool)> = vec![
        ("              .,,,,.              ", true),
        ("        .,'onNMMMMMNNnn',.        ", true),
        ("     .'oNMANKMMMMMMMMMMMNNn'.     ", true),
        ("   .'ANMMMMMMMXKNNWWWPFFWNNMNn.   ", true),
        ("  ;NNMMMMMMMMMMNWW'' ,.., 'WMMM,  ", true),
        (" ;NMMMMV+##+VNWWW' .+;'':+, 'WMW, ", true),
        (",VNNWP+######+WW,  +:    :+, +MMM,", true),
        ("'+#############,   +.    ,+' +NMMM", false),
        ("  '*#########*'     '*,,*' .+NMMMM", false),
        ("     `'*###*'          ,.,;###+WNM", false),
        ("         .,;;,      .;##########+W", false),
        (",',.         ';  ,+##############'", false),
        (" '###+. :,. .,; ,###############' ", false),
        ("  '####.. `'' .,###############'  ", false),
        ("    '#####+++################'    ", false),
        ("      '*##################*'      ", false),
        ("         ''*##########*''         ", false),
        ("              ''''''              ", false),
    ];

    lines
        .into_iter()
        .map(|(text, is_magenta)| {
            let padded = format!("{:<width$}", text, width = width);
            if is_magenta {
                format!("{}", padded.magenta())
            } else {
                format!("{}", padded.white())
            }
        })
        .collect()
}

fn logo_width() -> usize {
    35
}

fn build_info_lines(stats: &SteamStats) -> Vec<String> {
    let mut lines = vec![
        format!("{}@{}", stats.username.bold().cyan(), "Steam".bold().cyan()),
        "─".repeat(25),
        format!(
            "{}: {} ({} unplayed)",
            "Games".bold(),
            stats.game_count,
            stats.unplayed_count
        ),
        format!(
            "{}: {}h ({} days)",
            "Playtime".bold(),
            format_number(stats.playtime_hours()),
            stats.playtime_days()
        ),
    ];

    if let Some(ref achievements) = stats.achievement_stats {
        let percent = (achievements.total_achieved as f64 / achievements.total_possible as f64
            * 100.0) as u32;
        lines.push(format!(
            "{}: {} games",
            "Perfect".bold(),
            achievements.perfect_games
        ));
        lines.push(format!(
            "{}: {} / {} ({}%)",
            "Achievements".bold(),
            format_number(achievements.total_achieved),
            format_number(achievements.total_possible),
            percent
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
