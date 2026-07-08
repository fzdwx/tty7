use std::sync::OnceLock;

use log::{Level, LevelFilter, Metadata, Record};

static LOGGER: StderrLogger = StderrLogger;
static DIRECTIVES: OnceLock<Vec<Directive>> = OnceLock::new();

struct StderrLogger;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Directive {
    target: Option<String>,
    level: LevelFilter,
}

pub fn init() {
    let directives = directives();
    let max_level = directives
        .iter()
        .map(|directive| directive.level)
        .max()
        .unwrap_or(LevelFilter::Off);
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(max_level);
}

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        enabled(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        eprintln!("[{} {}] {}", record.level(), record.target(), record.args());
    }

    fn flush(&self) {}
}

fn enabled(target: &str, level: Level) -> bool {
    directives().iter().any(|directive| {
        level_enabled(level, directive.level)
            && directive
                .target
                .as_deref()
                .is_none_or(|prefix| target.starts_with(prefix))
    })
}

fn directives() -> &'static [Directive] {
    DIRECTIVES
        .get_or_init(|| parse_rust_log(&std::env::var("RUST_LOG").unwrap_or_default()))
        .as_slice()
}

fn parse_rust_log(value: &str) -> Vec<Directive> {
    value
        .split(',')
        .filter_map(|part| parse_directive(part.trim()))
        .collect()
}

fn parse_directive(part: &str) -> Option<Directive> {
    if part.is_empty() {
        return None;
    }

    let (target, level) = match part.split_once('=') {
        Some((target, level)) => (Some(target.trim()), level.trim()),
        None => (None, part),
    };
    let level = parse_level(level)?;
    let target = target
        .filter(|target| !target.is_empty())
        .map(str::to_string);
    Some(Directive { target, level })
}

fn parse_level(level: &str) -> Option<LevelFilter> {
    match level.to_ascii_lowercase().as_str() {
        "off" => Some(LevelFilter::Off),
        "error" => Some(LevelFilter::Error),
        "warn" | "warning" => Some(LevelFilter::Warn),
        "info" => Some(LevelFilter::Info),
        "debug" => Some(LevelFilter::Debug),
        "trace" => Some(LevelFilter::Trace),
        _ => None,
    }
}

fn level_enabled(level: Level, filter: LevelFilter) -> bool {
    filter.to_level().is_some_and(|max| level <= max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_global_and_targeted_directives() {
        assert_eq!(
            parse_rust_log("warn,tty7::file_tree=trace"),
            vec![
                Directive {
                    target: None,
                    level: LevelFilter::Warn,
                },
                Directive {
                    target: Some("tty7::file_tree".to_string()),
                    level: LevelFilter::Trace,
                },
            ]
        );
    }

    #[test]
    fn filters_by_prefix_and_level() {
        let directive = Directive {
            target: Some("tty7::file_tree".to_string()),
            level: LevelFilter::Debug,
        };

        assert!(matches_directive(
            &directive,
            "tty7::file_tree::cache",
            Level::Debug
        ));
        assert!(!matches_directive(
            &directive,
            "tty7::workspace",
            Level::Debug
        ));
        assert!(!matches_directive(
            &directive,
            "tty7::file_tree",
            Level::Trace
        ));
    }

    fn matches_directive(directive: &Directive, target: &str, level: Level) -> bool {
        level_enabled(level, directive.level)
            && directive
                .target
                .as_deref()
                .is_none_or(|prefix| target.starts_with(prefix))
    }
}
