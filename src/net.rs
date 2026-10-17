//! HTTPS через WinHTTP из Windows: системные прокси и сертификаты, без своих
//! библиотек TLS. Используется для обновлений (GitHub) и Ask Jiyda (Anthropic API).

use std::time::Duration;

/// Ответ сервера: код и тело.
pub struct Response {
    pub status: u32,
    pub body: Vec<u8>,
}

/// HTTPS-запрос. `headers` — пары (имя, значение); тело ответа длиннее `max_bytes` —
/// ошибка. Ответ с любым кодом — Ok: код проверяет вызывающий.
#[cfg(windows)]
pub fn https(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    max_bytes: usize,
    timeout: Duration,
) -> Result<Response, String> {
    use std::ffi::c_void;
    use windows::Win32::Networking::WinHttp::{
        INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ADDREQ_FLAG_ADD, WINHTTP_FLAG_SECURE,
        WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpAddRequestHeaders, WinHttpCloseHandle,
        WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
        WinHttpSendRequest, WinHttpSetTimeouts,
    };
    use windows::core::PCWSTR;

    /// Дескриптор WinHTTP, который закрывается сам.
    struct Handle(*mut c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: дескриптор получен от WinHTTP и закрывается один раз.
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
    let open = |h: *mut c_void, what: &str| {
        if h.is_null() { Err(format!("{what}: {}", windows::core::Error::from_thread())) } else { Ok(Handle(h)) }
    };
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();

    let u = url::Url::parse(url).map_err(|e| e.to_string())?;
    if u.scheme() != "https" {
        return Err("только https".into());
    }
    let host = wide(u.host_str().ok_or("в адресе нет сервера")?);
    let mut path = u.path().to_string();
    if let Some(q) = u.query() {
        path.push('?');
        path.push_str(q);
    }
    let agent = wide(&format!("Jiyda/{}", env!("CARGO_PKG_VERSION")));
    let (path, verb) = (wide(&path), wide(method));
    let ms = timeout.as_millis().min(i32::MAX as u128) as i32;

    // SAFETY: последовательность вызовов WinHTTP с живыми дескрипторами и строками,
    // завершёнными нулём; буферы — свои и нужного размера.
    unsafe {
        let session = open(
            WinHttpOpen(PCWSTR(agent.as_ptr()), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0),
            "WinHttpOpen",
        )?;
        let _ = WinHttpSetTimeouts(session.0, ms, ms, ms, ms);
        let port = u.port().unwrap_or(INTERNET_DEFAULT_HTTPS_PORT);
        let connect = open(WinHttpConnect(session.0, PCWSTR(host.as_ptr()), port, 0), "соединение")?;
        let request = open(
            WinHttpOpenRequest(
                connect.0,
                PCWSTR(verb.as_ptr()),
                PCWSTR(path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            ),
            "запрос",
        )?;
        for (name, value) in headers {
            let line: Vec<u16> = format!("{name}: {value}").encode_utf16().collect();
            WinHttpAddRequestHeaders(request.0, &line, WINHTTP_ADDREQ_FLAG_ADD)
                .map_err(|e| format!("заголовок {name}: {e}"))?;
        }
        let (ptr, len) = body.map_or((None, 0), |b| (Some(b.as_ptr() as *const c_void), b.len() as u32));
        WinHttpSendRequest(request.0, None, ptr, len, len, 0).map_err(|e| format!("отправка запроса: {e}"))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| format!("ответ сервера: {e}"))?;
        let (mut status, mut slen) = (0u32, 4u32);
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut c_void),
            &mut slen,
            std::ptr::null_mut(),
        )
        .map_err(|e| format!("код ответа: {e}"))?;
        let mut out = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            WinHttpReadData(request.0, buf.as_mut_ptr() as *mut c_void, buf.len() as u32, &mut read)
                .map_err(|e| format!("чтение ответа: {e}"))?;
            if read == 0 {
                break;
            }
            out.extend_from_slice(&buf[..read as usize]);
            if out.len() > max_bytes {
                return Err("ответ слишком большой".into());
            }
        }
        Ok(Response { status, body: out })
    }
}

#[cfg(not(windows))]
pub fn https(
    _method: &str,
    _url: &str,
    _headers: &[(&str, &str)],
    _body: Option<&[u8]>,
    _max_bytes: usize,
    _timeout: Duration,
) -> Result<Response, String> {
    Err("сеть через WinHTTP есть только в Windows".into())
}
