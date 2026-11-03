//! `?url=` / `?src=`: open a workbook from an address when the page starts (`docs/viewer.md`,
//! `packaging/web/README.md`). The browser side (`web::open_url`) resolves the address against
//! the page, fetches it and queues the bytes in the inbox as a dropped file. These are the parts
//! that need no browser: which parameter wins, which schemes may be fetched, the size limit and
//! what the workbook is called.

/// What a workbook is called when nothing names it.
pub const FALLBACK_NAME: &str = "workbook.xlsx";
/// Longest workbook name kept from `?name=`, `Content-Disposition` or the address.
const MAX_NAME_CHARS: usize = 200;
/// Largest workbook fetched from an address (the bytes are held in memory, as a dropped file's).
pub const MAX_BYTES: u64 = 512 << 20;

/// The address to open: `?url=`, else its alias `?src=`. Blank values count as missing.
pub fn requested(url: Option<String>, src: Option<String>) -> Option<String> {
    [url, src].into_iter().flatten().map(|s| s.trim().to_string()).find(|s| !s.is_empty())
}

/// Whether an address with this scheme may be fetched (`https:`, `http`, `HTTPS:` as
/// `URL.protocol` gives it). Only web addresses and the page's own blobs: never `javascript:`,
/// and not `data:` or `file:`.
pub fn check_scheme(scheme: &str) -> Result<(), String> {
    let scheme = scheme.trim().trim_end_matches(':').to_ascii_lowercase();
    match scheme.as_str() {
        "http" | "https" | "blob" => Ok(()),
        "" => Err("the address has no scheme".into()),
        other => Err(format!("{other}: addresses can't be opened (only http, https and blob)")),
    }
}

/// Whether a download of `len` bytes (from `Content-Length`, or as read) is too large.
pub fn too_large(len: u64) -> bool {
    len > MAX_BYTES
}

/// The message for a failed HTTP response.
pub fn http_error(status: u16, text: &str) -> String {
    let text = text.trim();
    let hint = match status {
        401 | 403 => " (the server refused access)",
        404 | 410 => " (no file at that address)",
        _ => "",
    };
    if text.is_empty() { format!("HTTP {status}{hint}") } else { format!("HTTP {status} {text}{hint}") }
}

/// The name to show while the workbook downloads: `?name=`, else the address's last path
/// segment, else [`FALLBACK_NAME`].
pub fn initial_name(name: Option<&str>, address: &str) -> String {
    name.map(file_part).filter(|n| !n.is_empty()).or_else(|| address_name(address)).unwrap_or_else(|| FALLBACK_NAME.into())
}

/// The workbook's name once the response is in: `?name=`, else the `Content-Disposition`
/// file name, else the address's last path segment (query and fragment dropped,
/// percent-decoded), else [`FALLBACK_NAME`]. Only a file name is kept. A name without a known
/// spreadsheet extension gets one from `Content-Type` when that says which (the engine also tells
/// xlsx, xls, xlsb and ods apart by their content).
pub fn final_name(name: Option<&str>, disposition: Option<&str>, content_type: Option<&str>, address: &str) -> String {
    let n = name
        .map(file_part)
        .filter(|n| !n.is_empty())
        .or_else(|| disposition.and_then(disposition_filename).map(|n| file_part(&n)).filter(|n| !n.is_empty()))
        .or_else(|| address_name(address));
    let ext = content_type.and_then(extension_for_type);
    match n {
        Some(n) if has_known_extension(&n) => n,
        Some(n) => match ext {
            Some(ext) => format!("{n}.{ext}"),
            None => n,
        },
        None => match ext {
            Some(ext) => format!("workbook.{ext}"),
            None => FALLBACK_NAME.into(),
        },
    }
}

/// The spreadsheet extensions GridCraft opens (`gridcraft_engine::io::FileKind`).
fn has_known_extension(name: &str) -> bool {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    matches!(
        ext.as_str(),
        "xlsx" | "xlsm" | "xltx" | "xltm" | "xls" | "xlt" | "xlsb" | "ods" | "csv" | "tsv" | "tab" | "txt" | "json" | "html" | "htm"
    )
}

/// The extension a `Content-Type` stands for, if it is a spreadsheet type.
pub fn extension_for_type(content_type: &str) -> Option<&'static str> {
    let mime = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    Some(match mime.as_str() {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.ms-excel.sheet.macroenabled.12" => "xlsm",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.template" => "xltx",
        "application/vnd.ms-excel.sheet.binary.macroenabled.12" => "xlsb",
        "application/vnd.ms-excel" => "xls",
        "application/vnd.oasis.opendocument.spreadsheet" => "ods",
        "text/csv" | "application/csv" => "csv",
        "text/tab-separated-values" => "tsv",
        _ => return None,
    })
}

/// The file name in a `Content-Disposition` header: `filename*=UTF-8''…` (RFC 5987) wins over
/// `filename="…"` or `filename=…`.
pub fn disposition_filename(header: &str) -> Option<String> {
    let mut plain = None;
    let mut extended = None;
    for part in split_params(header) {
        let Some((key, value)) = part.split_once('=') else { continue };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if key == "filename*" {
            // charset'language'percent-encoded
            let mut it = value.splitn(3, '\'');
            if let (Some(charset), Some(_lang), Some(encoded)) = (it.next(), it.next(), it.next()) {
                let bytes = percent_decode_bytes(encoded.trim_matches('"'));
                extended = Some(if charset.eq_ignore_ascii_case("utf-8") {
                    String::from_utf8_lossy(&bytes).into_owned()
                } else {
                    // ISO-8859-1: every byte is the code point.
                    bytes.iter().map(|&b| b as char).collect()
                });
            }
        } else if key == "filename" {
            plain = Some(unquote(value));
        }
    }
    extended.or(plain).filter(|s| !s.trim().is_empty())
}

/// `;`-separated parameters, not splitting inside quotes.
fn split_params(s: &str) -> Vec<String> {
    let (mut parts, mut cur, mut quoted, mut escaped) = (Vec::new(), String::new(), false, false);
    for c in s.chars() {
        match c {
            _ if escaped => {
                cur.push(c);
                escaped = false;
            }
            '\\' if quoted => {
                cur.push(c);
                escaped = true;
            }
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    parts
}

fn unquote(v: &str) -> String {
    match v.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Some(inner) => {
            let mut out = String::new();
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        out.push(n);
                    }
                } else {
                    out.push(c);
                }
            }
            out
        }
        None => v.to_string(),
    }
}

/// The address's last path segment, decoded, if it names something.
fn address_name(address: &str) -> Option<String> {
    let path = address.split(['?', '#']).next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("");
    Some(file_part(&String::from_utf8_lossy(&percent_decode_bytes(last)))).filter(|n| !n.is_empty())
}

/// The part after the last `/` or `\`, without control characters, trimmed and capped.
fn file_part(s: &str) -> String {
    let last = s.rsplit(['/', '\\']).next().unwrap_or("");
    let clean: String = last.chars().filter(|c| !c.is_control()).take(MAX_NAME_CHARS).collect();
    let clean = clean.trim();
    if clean == "." || clean == ".." { String::new() } else { clean.to_string() }
}

fn percent_decode_bytes(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && let (Some(hi), Some(lo)) = (b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex))
        {
            out.push((hi * 16 + lo) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_wins_over_src_and_blanks_are_missing() {
        assert_eq!(requested(Some("a.xlsx".into()), Some("b.xlsx".into())).as_deref(), Some("a.xlsx"));
        assert_eq!(requested(None, Some(" b.xlsx ".into())).as_deref(), Some("b.xlsx"));
        assert_eq!(requested(Some("  ".into()), Some("b.xlsx".into())).as_deref(), Some("b.xlsx"));
        assert_eq!(requested(Some(String::new()), None), None);
        assert_eq!(requested(None, None), None);
    }

    #[test]
    fn only_web_and_blob_schemes() {
        for ok in ["http:", "https:", "HTTPS:", "blob:", "https"] {
            assert!(check_scheme(ok).is_ok(), "{ok}");
        }
        for bad in ["javascript:", "JavaScript:", "data:", "file:", "ftp:", "about:", "chrome:", ""] {
            assert!(check_scheme(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_size_limit() {
        assert!(!too_large(MAX_BYTES));
        assert!(too_large(MAX_BYTES + 1));
    }

    #[test]
    fn http_errors_say_what_happened() {
        assert_eq!(http_error(404, "Not Found"), "HTTP 404 Not Found (no file at that address)");
        assert_eq!(http_error(403, ""), "HTTP 403 (the server refused access)");
        assert_eq!(http_error(500, "Internal Server Error"), "HTTP 500 Internal Server Error");
    }

    #[test]
    fn name_from_the_address() {
        let n = |a: &str| final_name(None, None, None, a);
        assert_eq!(n("https://example.com/files/report.xlsx"), "report.xlsx");
        assert_eq!(n("https://example.com/a/Q3%20plan%20%C3%A9t%C3%A9.ods?token=1#p2"), "Q3 plan été.ods");
        assert_eq!(n("/sales.csv"), "sales.csv");
        assert_eq!(n("https://example.com/download?id=5"), "download");
        assert_eq!(n("https://example.com/"), FALLBACK_NAME);
        assert_eq!(n("https://example.com/x/..%2F..%2Fevil.xls"), "evil.xls");
        assert_eq!(n("https://example.com/%2E%2E"), FALLBACK_NAME);
        assert_eq!(n("https://example.com/bad%zz.csv"), "bad%zz.csv");
        assert_eq!(n("blob:https://example.com/5b1c-77aa"), "5b1c-77aa");
        assert_eq!(initial_name(None, "https://example.com/files/report.xlsx"), "report.xlsx");
        assert_eq!(initial_name(None, "https://example.com/"), FALLBACK_NAME);
    }

    #[test]
    fn name_param_wins_and_is_only_a_file_name() {
        let n = |name: &str| final_name(Some(name), Some("attachment; filename=other.xls"), None, "https://example.com/y.csv");
        assert_eq!(n("Budget.xlsx"), "Budget.xlsx");
        assert_eq!(n("../../etc/passwd"), "passwd");
        assert_eq!(n("C:\\dir\\a.xls"), "a.xls");
        assert_eq!(n("  "), "other.xls");
        assert_eq!(n("a\u{0}b\n.csv"), "ab.csv");
        assert_eq!(n(&"x".repeat(500)).chars().count(), MAX_NAME_CHARS);
        assert_eq!(initial_name(Some("Budget.xlsx"), "/y.csv"), "Budget.xlsx");
    }

    #[test]
    fn content_disposition_names_the_workbook() {
        assert_eq!(disposition_filename("attachment; filename=\"Q3 sales.xlsx\"").as_deref(), Some("Q3 sales.xlsx"));
        assert_eq!(disposition_filename("inline; filename=data.csv").as_deref(), Some("data.csv"));
        assert_eq!(disposition_filename("attachment; filename=\"a;b.ods\"").as_deref(), Some("a;b.ods"));
        assert_eq!(disposition_filename("attachment; filename=\"say \\\"hi\\\".xls\"").as_deref(), Some("say \"hi\".xls"));
        assert_eq!(
            disposition_filename("attachment; filename=\"fallback.xlsx\"; filename*=UTF-8''%E2%82%AC%20rates.xlsx").as_deref(),
            Some("€ rates.xlsx")
        );
        assert_eq!(disposition_filename("attachment; FILENAME*=iso-8859-1'en'caf%E9.csv").as_deref(), Some("café.csv"));
        assert_eq!(disposition_filename("attachment"), None);
        assert_eq!(disposition_filename("attachment; filename=\"\""), None);
        // Wins over the address, and a path in it is cut to the file name.
        assert_eq!(final_name(None, Some("attachment; filename=\"../x/Report.xlsx\""), None, "/download?id=5"), "Report.xlsx");
    }

    #[test]
    fn content_type_supplies_a_missing_extension() {
        let xlsx = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
        assert_eq!(final_name(None, None, Some(xlsx), "https://example.com/download"), "download.xlsx");
        assert_eq!(final_name(None, None, Some("text/csv; charset=utf-8"), "https://example.com/export"), "export.csv");
        assert_eq!(final_name(None, None, Some("application/vnd.ms-excel"), "https://example.com/"), "workbook.xls");
        assert_eq!(final_name(None, None, Some("application/vnd.oasis.opendocument.spreadsheet"), "/get?f=1"), "get.ods");
        // A known extension is kept, whatever the type says.
        assert_eq!(final_name(None, None, Some("application/octet-stream"), "/a.xlsb"), "a.xlsb");
        assert_eq!(final_name(None, None, Some("text/csv"), "/a.xlsx"), "a.xlsx");
        // An unknown type adds nothing; the engine tells the format from the content.
        assert_eq!(final_name(None, None, Some("application/octet-stream"), "/blob-id"), "blob-id");
        assert_eq!(extension_for_type("TEXT/TAB-SEPARATED-VALUES"), Some("tsv"));
        assert_eq!(extension_for_type("application/vnd.ms-excel.sheet.macroEnabled.12"), Some("xlsm"));
        assert_eq!(extension_for_type("text/html"), None);
    }
}
