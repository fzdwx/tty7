use gpui_component::IconName;

pub(super) fn short_title(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let after_host = match raw.split_once(':') {
        Some((head, tail)) if head.contains('@') => tail,
        _ => raw,
    };
    let path = after_host.trim();
    let name = if path == "~" {
        "~"
    } else {
        path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(path)
    };
    let mut name = name.to_string();
    if name.chars().count() > 24 {
        name = format!("{}…", name.chars().take(24).collect::<String>());
    }
    name
}

pub(super) fn icon_for_title(raw: &str) -> IconName {
    let label = short_title(raw);
    let cmd = label
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match cmd.as_str() {
        "ssh" | "mosh" => IconName::Globe,
        "git" | "lazygit" | "gitui" => IconName::Github,
        _ => IconName::SquareTerminal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_title_strips_user_host_and_keeps_last_segment() {
        assert_eq!(short_title("user@host:~/projects/app"), "app");
        assert_eq!(short_title("/usr/local/bin"), "bin");
        assert_eq!(short_title("plain"), "plain");
    }

    #[test]
    fn short_title_keeps_home_tilde_and_handles_trailing_slash() {
        assert_eq!(short_title("user@host:~"), "~");
        assert_eq!(short_title("~"), "~");
        assert_eq!(short_title("a/b/c/"), "c");
    }

    #[test]
    fn short_title_blank_input_is_empty_and_long_names_are_clamped() {
        assert_eq!(short_title("   "), "");
        let long = "a".repeat(40);
        let out = short_title(&long);
        assert_eq!(out.chars().count(), 25);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn icon_for_title_maps_known_commands_else_terminal() {
        assert!(matches!(icon_for_title("ssh box"), IconName::Globe));
        assert!(matches!(icon_for_title("git status"), IconName::Github));
        assert!(matches!(
            icon_for_title("vim file"),
            IconName::SquareTerminal
        ));
    }
}
