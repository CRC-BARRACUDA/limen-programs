//! The module itself — what it holds between calls, and what each call does.

use crate::*;

#[derive(Default)]
pub(crate) struct Programs {
    /// Whether the user has scanned this session. Once true, reopening the tab
    /// shows the saved results instead of the landing Scan button.
    scanned: bool,
    /// The raw search text from the last scan (restored into the search box).
    last_query: String,
    /// The page (0-based) currently shown, so reopening the tab or a row action
    /// returns to the same page rather than jumping back to the first.
    last_page: usize,
    /// The full entry list from the last scan, so the view can be re-rendered on
    /// reopen without re-enumerating the machine.
    last_entries: Vec<Value>,
    /// The last scan, keyed by the row id sent back on a row action, so `about`
    /// / `open_location` can resolve which entry the user acted on.
    last: HashMap<String, Value>,
}

impl Handler for Programs {
    fn capabilities(&self) -> Vec<String> {
        vec!["programs.local".into()]
    }

    fn invoke(
        &mut self,
        _capability: &str,
        method: &str,
        params: Value,
        host: &Host,
    ) -> Result<Value, RpcError> {
        // Optional integration: only offer "Make Report" when a report provider
        // is actually loaded (discovered at call time, never a hard dependency).
        let report = host.has_capability("report.build");
        // One lookup per call, then every view renders in that language.
        let lang = host.locale();
        let lang = lang.as_str();
        match method {
            // Landing view: the saved results if the user has scanned this
            // session, otherwise just a Scan button (no enumeration on open).
            "ui" => Ok(if self.scanned {
                let entries = self.last_entries.clone();
                let query = self.last_query.clone();
                self.render(lang, &entries, &query, self.last_page, report)
            } else {
                idle_view(lang)
            }),
            // Scan now (enumerate), save the state, and render (also Refresh).
            "scan" => Ok(self.scan(lang, &params, report)),
            // Turn to a page (from a pager button) without re-enumerating.
            "page" => Ok(self.page(lang, &params, report)),
            "list" => Ok(list_programs()),
            // Row actions: open an entry's details, or open where it's installed.
            "about" => Ok(self.about(lang, &params)),
            "open_location" => Ok(self.open_location(&params, host)),
            "reveal" => Ok(self.reveal(&params, host)),
            "copy_path" => Ok(self.copy_path(&params, host)),
            // Report integration (present only while a report provider is loaded).
            "report_config" => Ok(report_config(lang)),
            "make_report" => Ok(self.make_report(lang, &params, host)),
            other => Err(RpcError::new(
                rpc::METHOD_NOT_FOUND,
                format!("programs has no method {other}"),
            )),
        }
    }
}

impl Programs {
    /// Enumerate the machine, save the scan state (so reopening the tab restores
    /// it), and render. `params.query` filters; Refresh calls this again.
    fn scan(&mut self, lang: &str, params: &Value, report: bool) -> Value {
        let query = params
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let data = list_programs();
        let entries: Vec<Value> = data
            .get("entries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        self.scanned = true;
        self.last_entries = entries.clone();
        self.last_query = query.clone();
        // A fresh scan / search resets to the first page.
        self.render(lang, &entries, &query, 0, report)
    }

    /// Turn to another page of the current (already-scanned) results. Re-slices
    /// `last_entries` — no re-enumeration — honoring the live search box so paging
    /// stays within the filtered set. `params`: `{ query, page }`.
    fn page(&mut self, lang: &str, params: &Value, report: bool) -> Value {
        let query = params
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let page = params.get("page").and_then(Value::as_u64).unwrap_or(0) as usize;
        let entries = self.last_entries.clone();
        self.last_query = query.clone();
        self.render(lang, &entries, &query, page, report)
    }

    /// The results view: filter `entries` by `query_raw`, slice out the current
    /// page, and cache each shown entry by its row id so row actions
    /// (`about` / `open_location`) resolve it.
    pub(crate) fn render(
        &mut self,
        lang: &str,
        entries: &[Value],
        query_raw: &str,
        page: usize,
        report: bool,
    ) -> Value {
        let query = query_raw.to_lowercase();
        let matches =
            |d: &Value| query.is_empty() || row_cells(d).join(" ").to_lowercase().contains(&query);

        // Filter first, keeping each entry's original index so row ids stay stable
        // across pages (about/open_location resolve by that index).
        let filtered: Vec<(usize, &Value)> =
            entries.iter().enumerate().filter(|(_, d)| matches(d)).collect();
        let total = filtered.len();
        let page_count = if total == 0 { 1 } else { total.div_ceil(PAGE_SIZE) };
        let page = page.min(page_count - 1);
        self.last_page = page;
        let start = page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(total);

        // Only the current page is shown, so cache only its rows for row actions.
        self.last.clear();
        let (mut rows, mut ids, mut menus) = (Vec::new(), Vec::new(), Vec::new());
        for (orig, d) in &filtered[start..end] {
            let rid = orig.to_string();
            self.last.insert(rid.clone(), (*d).clone());
            ids.push(rid);
            rows.push(row_cells(d));
            menus.push(row_menu_for(lang, d)); // per-row: open action only when openable
        }

        results_view(
            lang,
            query_raw,
            rows,
            ids,
            menus,
            (start, end, total),
            (page, page_count),
            report,
        )
    }

    /// A detail view for one entry (opened in a new tab from a row action).
    fn about(&self, lang: &str, params: &Value) -> Value {
        let id = params.get("id").and_then(Value::as_str).unwrap_or("");
        match self.last.get(id) {
            Some(d) => about_view(lang, d, id),
            None => stale_view(lang),
        }
    }

    /// Open where a program is installed: the folder or file in the file manager.
    /// `params`: `{ id }`.
    fn open_location(&self, params: &Value, host: &Host) -> Value {
        let id = params.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(d) = self.last.get(id) {
            if let Some((_, target, value)) = open_kind(d) {
                host.open(target, &value);
            }
        }
        Value::Null
    }

    /// Reveal the entry's location in the OS file manager (Explorer / Finder /
    /// Files) with the item selected. `params`: `{ id }`.
    fn reveal(&self, params: &Value, host: &Host) -> Value {
        let id = params.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(path) = self.last.get(id).and_then(resolved_path) {
            host.open("reveal", &path);
        }
        Value::Null
    }

    /// Copy the entry's path (install location / entry file, or a resolved
    /// package file) to the system clipboard. `params`: `{ id }`.
    fn copy_path(&self, params: &Value, host: &Host) -> Value {
        let id = params.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(path) = self.last.get(id).and_then(resolved_path) {
            if copy_to_clipboard(&path) {
                host.log(&format!("programs: copied path to clipboard: {path}"));
            } else {
                host.log("programs: couldn't copy path — no clipboard tool found");
            }
        }
        Value::Null
    }

    /// Build a report spec from the last scan and hand it to a report provider.
    fn make_report(&self, lang: &str, params: &Value, host: &Host) -> Value {
        let sent = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("");
        let fmt = choice(lang, &FORMATS, sent("format"));
        let content = choice(lang, &CONTENT, sent("content"));
        let scope = choice(lang, &SCOPES, sent("scope"));
        let spec = report_spec(lang, &self.last_entries, fmt, content, scope);
        match host.call("report.build", "build", spec) {
            // The provider answered with a view of its own — it is the report.
            Ok(v) if v.get("widgets").is_some() => v,
            Ok(_) => exported_view(lang),
            Err(e) => report_failed_view(lang, format!("{e}")),
        }
    }
}
