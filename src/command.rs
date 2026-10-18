//! Команды браузера: горячие клавиши, меню, палитра и сообщения интерфейса вызывают
//! одно и то же. Имя команды — строка snake_case (её видят config.json и интерфейс);
//! внутри ядра — только `Command`, поэтому опечатка в имени не проходит молча.

use std::fmt;
use std::str::FromStr;

macro_rules! commands {
    ($($variant:ident => $name:literal),* $(,)?) => {
        /// Команда браузера.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
        #[serde(try_from = "String")]
        pub enum Command {
            $($variant,)*
            /// Space по номеру с 1 (Ctrl+Shift+1…9), имя `space:N`.
            Space(usize),
            /// Страница по номеру с 1 (Ctrl+1…9; 9 — последняя), имя `goto:N`.
            Goto(usize),
        }

        impl Command {
            /// Все команды без номера — для проверок и списков.
            #[cfg(test)]
            pub const ALL: &[Command] = &[$(Command::$variant,)*];
        }

        impl fmt::Display for Command {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Command::$variant => f.write_str($name),)*
                    Command::Space(n) => write!(f, "space:{n}"),
                    Command::Goto(n) => write!(f, "goto:{n}"),
                }
            }
        }

        impl FromStr for Command {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let number = |prefix: &str| s.strip_prefix(prefix).and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=9).contains(n));
                if let Some(n) = number("space:") {
                    return Ok(Command::Space(n));
                }
                if let Some(n) = number("goto:") {
                    return Ok(Command::Goto(n));
                }
                match s {
                    $($name => Ok(Command::$variant),)*
                    _ => Err(format!("неизвестная команда: {s}")),
                }
            }
        }
    };
}

commands! {
    // Слои интерфейса.
    Palette => "palette",
    Orbit => "orbit",
    Spaces => "spaces",
    History => "history",
    Closed => "closed",
    Bookmarks => "bookmarks",
    Ask => "ask",
    Settings => "settings",
    Shortcuts => "shortcuts",
    Focus => "focus",
    Downloads => "downloads",
    EditUrl => "edit_url",
    Find => "find",
    // Страницы.
    NewTab => "new_tab",
    Close => "close",
    Reopen => "reopen",
    Next => "next",
    Prev => "prev",
    NextSeq => "next_seq",
    PrevSeq => "prev_seq",
    Later => "later",
    Pin => "pin",
    Bookmark => "bookmark",
    Back => "back",
    Forward => "forward",
    Reload => "reload",
    HardReload => "hard_reload",
    Stop => "stop",
    CopyUrl => "copy_url",
    Clear => "clear",
    Home => "home",
    Devtools => "devtools",
    // Split View.
    Split => "split",
    Unsplit => "unsplit",
    SplitSwap => "split_swap",
    // Окно.
    Fullscreen => "fullscreen",
    WinMin => "win_min",
    WinMax => "win_max",
    WinClose => "win_close",
    // Spaces, данные, система.
    SpaceNew => "space_new",
    OpenDownloadsFolder => "open_downloads_folder",
    OpenDataFolder => "open_data_folder",
    Licenses => "licenses",
    ClearHistory => "clear_history",
    ClearData => "clear_data",
    ClearClosed => "clear_closed",
    DefaultBrowser => "default_browser",
    CheckUpdates => "check_updates",
    Theme => "theme",
    UpdateNow => "update_now",
    UpdateDismiss => "update_dismiss",
    Recover => "recover",
    RecoverDismiss => "recover_dismiss",
    ImportDismiss => "import_dismiss",
}

impl TryFrom<String> for Command {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl Command {
    /// При удержании клавиши повторяются только переключения страниц.
    pub fn repeats(self) -> bool {
        matches!(self, Command::Next | Command::Prev | Command::NextSeq | Command::PrevSeq)
    }
}

#[cfg(test)]
#[path = "../tests/unit/command.rs"]
mod tests;
