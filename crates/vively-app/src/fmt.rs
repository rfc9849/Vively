pub fn fmt_duration(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn fmt_duration_f64(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".into();
    }
    fmt_duration(seconds.floor() as i64)
}

pub fn fmt_total_duration(total_secs: i64) -> String {
    if total_secs <= 0 {
        return String::new();
    }
    if total_secs < 60 {
        return "< 1 мин".into();
    }
    let total_mins = (total_secs + 30) / 60;
    let hours = total_mins / 60;
    let mins = total_mins % 60;
    if hours == 0 {
        format!("{mins} мин")
    } else if mins == 0 {
        format!("{hours} ч")
    } else {
        format!("{hours} ч {mins} мин")
    }
}

fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

fn ru_plural<'a>(n: u64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let mod10 = n % 10;
    let mod100 = n % 100;
    if mod10 == 1 && mod100 != 11 {
        one
    } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
        few
    } else {
        many
    }
}

fn fmt_count(n: u64, one: &str, few: &str, many: &str) -> String {
    format!("{} {}", group_thousands(n), ru_plural(n, one, few, many))
}

pub fn fmt_tracks_count(count: i64) -> String {
    fmt_count(count.max(0) as u64, "трек", "трека", "треков")
}

pub fn fmt_plays_count(count: i64) -> String {
    fmt_count(
        count.max(0) as u64,
        "прослушивание",
        "прослушивания",
        "прослушиваний",
    )
}

pub fn fmt_playlists_count(count: usize) -> String {
    fmt_count(count as u64, "плейлист", "плейлиста", "плейлистов")
}

pub fn fmt_friends_count(count: i64) -> String {
    fmt_count(count.max(0) as u64, "друг", "друга", "друзей")
}

pub fn fmt_gb(bytes: u64) -> String {
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb < 0.05 && bytes > 0 {
        let mb = (bytes as f64 / (1024.0 * 1024.0)).max(0.1);
        format!("{:.1} МБ", mb).replace('.', ",")
    } else {
        format!("{:.1} ГБ", gb).replace('.', ",")
    }
}
