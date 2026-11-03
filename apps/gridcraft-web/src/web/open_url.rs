//! `?url=` / `?src=` in the browser: resolve the address against the page, fetch it and queue the
//! bytes in the inbox as a dropped file, so the app opens it as it opens any other (the engine
//! tells xlsx, xls, xlsb and ods apart by their content; `crate::url_open` checks the scheme and
//! names the workbook). While it downloads the viewer says *Opening <name>…*; a failed download
//! is shown in a message box, as a file that can't be opened is, and logged to the console.

use std::cell::RefCell;
use std::rc::Rc;

use gridcraft_ui_egui::{Inbox, SheetApp};
use wasm_bindgen::JsCast as _;

enum Phase {
    Fetching,
    /// In the inbox; the app opens it (or says why it can't) in its next frame.
    Queued,
    Failed {
        error: String,
        reported: bool,
    },
    Done,
}

struct State {
    name: String,
    phase: Phase,
}

/// A workbook being opened from an address.
#[derive(Clone)]
pub struct UrlOpen {
    state: Rc<RefCell<State>>,
    inbox: Inbox,
}

impl UrlOpen {
    /// Starts fetching `address`; `name` is `?name=`.
    pub fn start(address: &str, name: Option<&str>, inbox: Inbox, ctx: egui::Context) -> Self {
        let resolved = resolve(address);
        let shown = match &resolved {
            Ok(href) => crate::url_open::initial_name(name, href),
            Err(_) => crate::url_open::unfetchable_name(name),
        };
        let this = Self { state: Rc::new(RefCell::new(State { name: shown, phase: Phase::Fetching })), inbox: inbox.clone() };
        let href = match resolved {
            Ok(href) => href,
            Err(e) => {
                this.fail(e);
                return this;
            }
        };
        log::info!("opening a workbook from {href}");
        let state = this.clone();
        let name = name.map(str::to_string);
        wasm_bindgen_futures::spawn_local(async move {
            match fetch(&href, name.as_deref()).await {
                Ok((doc_name, bytes)) => {
                    log::info!("downloaded {doc_name} ({} bytes)", bytes.len());
                    state.state.borrow_mut().name = doc_name.clone();
                    inbox.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((doc_name, bytes));
                    state.state.borrow_mut().phase = Phase::Queued;
                }
                Err(e) => state.fail(e),
            }
            ctx.request_repaint();
        });
        this
    }

    fn fail(&self, error: String) {
        let mut s = self.state.borrow_mut();
        log::error!("{} (from ?url=)", crate::url_open::failure_message(&s.name, &error));
        s.phase = Phase::Failed { error, reported: false };
    }

    /// After the app's frame logic: a failed download is reported in a message box (once); a
    /// queued workbook has been handed to the app once the inbox is empty again.
    pub fn after_logic(&self, app: &mut SheetApp) {
        let mut s = self.state.borrow_mut();
        let name = s.name.clone();
        if let Phase::Failed { error, reported } = &mut s.phase {
            if !*reported {
                *reported = true;
                app.message = Some(("GridCraft Viewer".into(), crate::url_open::failure_message(&name, error)));
            }
        } else if matches!(s.phase, Phase::Queued) && self.inbox.lock().map(|q| q.is_empty()).unwrap_or(true) {
            s.phase = Phase::Done;
        }
    }

    /// While the workbook downloads: *Opening <name>…* over the sheet.
    pub fn show(&self, ctx: &egui::Context) {
        let s = self.state.borrow();
        if !matches!(s.phase, Phase::Fetching | Phase::Queued) {
            return;
        }
        egui::Area::new(egui::Id::unique("gridcraft_url_open"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(18, 12)).show(ui, |ui| {
                    ui.set_max_width((ctx.content_rect().width() - 48.0).clamp(160.0, 560.0));
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(egui::RichText::new(format!("Opening {}…", s.name)).size(14.0));
                    });
                });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// The absolute address, resolved against the page, if its scheme may be fetched.
fn resolve(address: &str) -> Result<String, String> {
    let base = web_sys::window().and_then(|w| w.location().href().ok()).unwrap_or_default();
    let url = web_sys::Url::new_with_base(address, &base).map_err(|_| format!("\u{201c}{address}\u{201d} isn't a valid address"))?;
    crate::url_open::check_scheme(&url.protocol())?;
    Ok(url.href())
}

/// Downloads `href`; the workbook's name and bytes.
async fn fetch(href: &str, name: Option<&str>) -> Result<(String, Vec<u8>), String> {
    let window = web_sys::window().ok_or("no window")?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    // Cookies for the viewer's own server only, never for another site's.
    init.set_credentials(web_sys::RequestCredentials::SameOrigin);
    let response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(href, &init)).await.map_err(|e| {
        log::error!("fetch {href}: {e:?}");
        "the download failed: no network, or the file is on another site whose server doesn't allow this one to read it \
         (it must send Access-Control-Allow-Origin)"
            .to_string()
    })?;
    let response: web_sys::Response = response.dyn_into().map_err(|_| "not a response".to_string())?;
    if !response.ok() {
        return Err(crate::url_open::http_error(response.status(), &response.status_text()));
    }
    let headers = response.headers();
    let header = |key: &str| headers.get(key).ok().flatten();
    if header("content-length").and_then(|l| l.trim().parse::<u64>().ok()).is_some_and(crate::url_open::too_large) {
        return Err("the file is larger than 512 MB".into());
    }
    let doc_name = crate::url_open::final_name(name, header("content-disposition").as_deref(), header("content-type").as_deref(), href);
    let promise = response.array_buffer().map_err(|e| format!("{e:?}"))?;
    let buffer = wasm_bindgen_futures::JsFuture::from(promise).await.map_err(|e| format!("the download was cut short: {e:?}"))?;
    let bytes = js_sys::Uint8Array::new(&buffer);
    if crate::url_open::too_large(u64::from(bytes.length())) {
        return Err("the file is larger than 512 MB".into());
    }
    if bytes.length() == 0 {
        return Err("the file is empty".into());
    }
    Ok((doc_name, bytes.to_vec()))
}
