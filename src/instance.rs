//! Один экземпляр Seyro и ссылки из других программ.
//!
//! Когда Seyro — браузер по умолчанию, Windows запускает `seyro.exe <адрес>` на каждую
//! ссылку. Если Seyro уже работает, новый процесс не открывает второе окно (два окна
//! на одной папке данных WebView2 — путь к её порче), а передаёт адреса работающему
//! через именованный канал и сразу завершается. Работающий открывает их во вкладках
//! и выходит на передний план. Пустое сообщение — просто «покажись» (повторный
//! запуск ярлыком).

use std::path::Path;

/// Сколько адресов за раз и какой длины принимаем — защита от мусора в канале.
const MAX_URLS: usize = 50;
const MAX_URL_LEN: usize = 8 * 1024;

/// Адрес из аргумента командной строки: ссылка как есть, путь к файлу — как file:///,
/// голый домен (`seyro.exe example.com`) — как https. Флаги и прочее — None.
pub fn arg_to_url(arg: &str, cwd: &Path) -> Option<String> {
    let arg = arg.trim().trim_matches('"');
    if arg.is_empty() || arg.starts_with('-') {
        return None;
    }
    if is_allowed(arg) {
        return Some(arg.to_string());
    }
    let path = cwd.join(arg);
    if path.exists() {
        return url::Url::from_file_path(std::fs::canonicalize(&path).unwrap_or(path))
            .ok()
            .map(|u| u.to_string());
    }
    // Голый домен — но не поисковый запрос: аргумент запуска искать в Google странно.
    let url = crate::input::resolve(arg, "")?;
    (url == format!("https://{arg}") || url == format!("http://{arg}")).then_some(url)
}

pub fn urls_from_args(args: impl IntoIterator<Item = String>, cwd: &Path) -> Vec<String> {
    args.into_iter()
        .filter_map(|a| arg_to_url(&a, cwd))
        .take(MAX_URLS)
        .collect()
}

/// Что можно открыть по сообщению из канала: обычные страницы и файлы, но не
/// javascript:, data: и прочее, что выполнилось бы как код.
fn is_allowed(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["http://", "https://", "file://", "about:"]
        .iter()
        .any(|s| lower.starts_with(s))
}

/// Сообщение в канал: адреса по одному на строку.
pub fn encode(urls: &[String]) -> Vec<u8> {
    let mut s = urls.join("\n");
    s.push('\n');
    s.into_bytes()
}

pub fn decode(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|u| !u.is_empty() && u.len() <= MAX_URL_LEN && is_allowed(u))
        .take(MAX_URLS)
        .map(str::to_string)
        .collect()
}

/// Имя канала: своё у каждого пользователя Windows (каналы видны всей машине).
fn pipe_name() -> String {
    let user: String = std::env::var("USERNAME")
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!(r"\\.\pipe\seyro-{user}")
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Передать адреса уже работающему Seyro. false — он не запущен, запускаемся сами.
#[cfg(windows)]
pub fn send_to_running(urls: &[String]) -> bool {
    use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_BUSY, GENERIC_WRITE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_NONE, OPEN_EXISTING, WriteFile,
    };
    use windows::Win32::System::Pipes::WaitNamedPipeW;
    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
    use windows::core::PCWSTR;

    let name = wide(&pipe_name());
    let open = || unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };
    // SAFETY: обычные вызовы Win32 с корректной строкой имени, завершённой нулём.
    unsafe {
        let handle = match open() {
            Ok(h) => h,
            // Работающий Seyro как раз обслуживает другой запуск — подождём.
            Err(e)
                if e.code() == ERROR_PIPE_BUSY.to_hresult()
                    && WaitNamedPipeW(PCWSTR(name.as_ptr()), 2000).as_bool() =>
            {
                match open() {
                    Ok(h) => h,
                    Err(_) => return false,
                }
            }
            Err(_) => return false,
        };
        // Разрешаем работающему окну выйти на передний план: Windows иначе не даст.
        let _ = AllowSetForegroundWindow(u32::MAX);
        let ok = WriteFile(handle, Some(&encode(urls)), None, None).is_ok();
        let _ = CloseHandle(handle);
        ok
    }
}

#[cfg(not(windows))]
pub fn send_to_running(_urls: &[String]) -> bool {
    false
}

/// Слушать сообщения от новых запусков в отдельном потоке.
#[cfg(windows)]
pub fn listen(on_message: impl Fn(Vec<String>) + Send + 'static) {
    use windows::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA, ERROR_PIPE_CONNECTED, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND, ReadFile,
    };
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows::core::PCWSTR;

    let name = wide(&pipe_name());
    let create = move |first: bool| -> Option<HANDLE> {
        let mut mode = PIPE_ACCESS_INBOUND;
        if first {
            // Если имя уже занято чужим процессом, не подключаемся к его каналу.
            mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        // SAFETY: создание канала со строкой имени, завершённой нулём; права по
        // умолчанию — писать в канал может только этот пользователь (и администраторы).
        let h = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                mode,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                0,
                64 * 1024,
                0,
                None,
            )
        };
        (!h.is_invalid()).then_some(h)
    };
    // Канал создаётся и живёт в своём потоке (дескриптор нельзя передавать между потоками).
    std::thread::spawn(move || {
        let Some(mut handle) = create(true) else {
            log::warn!("не удалось создать канал для ссылок из других программ");
            return;
        };
        loop {
            // SAFETY: handle — живой экземпляр канала; читаем в свой буфер.
            unsafe {
                let connected = match ConnectNamedPipe(handle, None) {
                    Ok(()) => true,
                    Err(e) => e.code() == ERROR_PIPE_CONNECTED.to_hresult(),
                };
                // Следующий экземпляр — заранее, чтобы новый запуск не застал канал пропавшим.
                let next = create(false);
                let mut message = Vec::new();
                if connected {
                    let mut buf = [0u8; 4096];
                    loop {
                        let mut read = 0u32;
                        let res = ReadFile(handle, Some(&mut buf), Some(&mut read), None);
                        message.extend_from_slice(&buf[..read as usize]);
                        match res {
                            Err(e)
                                if e.code() == ERROR_MORE_DATA.to_hresult()
                                    && message.len() < 1024 * 1024 =>
                            {
                                continue;
                            }
                            _ => break,
                        }
                    }
                }
                let _ = DisconnectNamedPipe(handle);
                let _ = CloseHandle(handle);
                if connected {
                    on_message(decode(&message));
                }
                match next {
                    Some(h) => handle = h,
                    None => {
                        log::warn!("канал для ссылок из других программ закрыт");
                        return;
                    }
                }
            }
        }
    });
}

#[cfg(not(windows))]
pub fn listen(_on_message: impl Fn(Vec<String>) + Send + 'static) {}

/// Имя, под которым установщик регистрирует Seyro в списке браузеров Windows
/// (HKCU\Software\RegisteredApplications, см. installer/seyro.iss).
const REGISTERED_NAME: &str = "Seyro";

/// Зарегистрирован ли Seyro как браузер — это делает установщик.
#[cfg(windows)]
pub fn registered_as_browser() -> bool {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::PCWSTR;
    let (key, value) = (
        wide(r"Software\RegisteredApplications"),
        wide(REGISTERED_NAME),
    );
    // SAFETY: только проверяем наличие значения; буфер не передаём.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
        .is_ok()
    }
}

#[cfg(not(windows))]
pub fn registered_as_browser() -> bool {
    false
}

/// Открыть в «Параметрах» страницу Seyro в «Приложениях по умолчанию». Назначить себя
/// браузером по умолчанию молча Windows 10/11 не дают — выбор делает пользователь.
/// Через ShellExecute: `explorer ms-settings:...` «Параметры» не открывает.
#[cfg(windows)]
pub fn open_default_apps_settings() -> bool {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;
    let uri = wide(&format!(
        "ms-settings:defaultapps?registeredAppUser={REGISTERED_NAME}"
    ));
    let verb = wide("open");
    // SAFETY: строки завершены нулём и живут до конца вызова.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(uri.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // По документации значение больше 32 — успех.
    result.0 as isize > 32
}

#[cfg(not(windows))]
pub fn open_default_apps_settings() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cwd() -> std::path::PathBuf {
        std::env::temp_dir()
    }

    #[test]
    fn links_pass_through() {
        for u in [
            "https://example.com/a?b=1",
            "http://x.test",
            "file:///C:/a.html",
            "about:blank",
            "HTTPS://X.TEST",
        ] {
            assert_eq!(arg_to_url(u, &cwd()).as_deref(), Some(u), "{u}");
        }
        assert_eq!(
            arg_to_url("\"https://q.test/\"", &cwd()).as_deref(),
            Some("https://q.test/")
        );
    }

    #[test]
    fn flags_and_junk_are_ignored() {
        for a in [
            "",
            "  ",
            "--flag",
            "-x",
            "hello world",
            "javascript:alert(1)",
            "data:text/html,x",
            "word",
        ] {
            assert_eq!(arg_to_url(a, &cwd()), None, "{a}");
        }
    }

    #[test]
    fn bare_domain_becomes_https() {
        assert_eq!(
            arg_to_url("example.com", &cwd()).as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            arg_to_url("localhost:3000", &cwd()).as_deref(),
            Some("http://localhost:3000")
        );
    }

    #[test]
    fn existing_file_becomes_file_url() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("page one.html"), "<p>x</p>").unwrap();
        let url = arg_to_url("page one.html", dir.path()).unwrap();
        assert!(
            url.starts_with("file:///") && url.ends_with("/page%20one.html"),
            "{url}"
        );
        let abs = dir.path().join("page one.html");
        assert!(
            arg_to_url(abs.to_str().unwrap(), &cwd())
                .unwrap()
                .starts_with("file:///")
        );
    }

    #[test]
    fn urls_from_args_keeps_order_and_skips_junk() {
        let args = ["--x", "https://a.test", "nothing here", "b.test"].map(String::from);
        assert_eq!(
            urls_from_args(args, &cwd()),
            ["https://a.test", "https://b.test"]
        );
    }

    #[test]
    fn message_roundtrip() {
        let urls = vec![
            "https://a.test/".to_string(),
            "file:///C:/x.html".to_string(),
        ];
        assert_eq!(decode(&encode(&urls)), urls);
        assert!(decode(&encode(&[])).is_empty());
    }

    #[test]
    fn decode_drops_dangerous_and_oversized() {
        let long = format!("https://a.test/{}", "x".repeat(MAX_URL_LEN));
        let msg =
            format!("javascript:alert(1)\r\ndata:text/html,<b>\n  https://ok.test/  \n{long}\n\n");
        assert_eq!(decode(msg.as_bytes()), ["https://ok.test/"]);
        let many: String = (0..100).map(|i| format!("https://t.test/{i}\n")).collect();
        assert_eq!(decode(many.as_bytes()).len(), MAX_URLS);
        assert!(decode(&[0xff, 0xfe, b'\n']).is_empty());
    }

    #[test]
    fn pipe_name_is_safe() {
        let n = pipe_name();
        assert!(n.starts_with(r"\\.\pipe\seyro-"));
        assert!(
            n[r"\\.\pipe\".len()..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }
}
