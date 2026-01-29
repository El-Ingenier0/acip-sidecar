use aho_corasick::AhoCorasick;

#[derive(Debug, Clone, Default)]
pub struct XmlScanResult {
    pub has_doctype: bool,
    pub has_entity: bool,
    pub has_external_ref: bool,
    pub has_scriptish: bool,
    pub matches: Vec<String>,

    /// Simple heuristic score for "this XML is suspicious / potentially dangerous".
    /// This is NOT a security boundary; sandboxing + limits remain the real defense.
    pub severity: u8,
}

static PATTERNS: &[(&str, &str)] = &[
    // DTD / entity expansion / XXE markers
    ("doctype", "<!doctype"),
    ("entity", "<!entity"),
    ("dtd_system", " system"),
    ("dtd_public", " public"),
    // External refs (often used for tracking/exfil, or just as an indicator)
    ("href", "href="),
    ("xlink_href", "xlink:href="),
    ("src", "src="),
    ("http", "http://"),
    ("https", "https://"),
    // Script-ish indicators (SVG/event handlers)
    ("script_tag", "<script"),
    ("onload", "onload="),
    ("onerror", "onerror="),
    ("javascript", "javascript:"),
];

fn matcher() -> AhoCorasick {
    // Case-insensitive, ASCII.
    let pats: Vec<&str> = PATTERNS.iter().map(|(_, p)| *p).collect();
    AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build(pats)
        .expect("aho-corasick patterns must compile")
}

/// Cheap pre-parse scan of XML-ish input to flag common red flags.
///
/// This is intentionally shallow: it looks for well-known tokens like `<!DOCTYPE` / `<!ENTITY`
/// and obvious external references. It should run before any XML parsing in the sandbox helper.
pub fn scan(input: &str) -> XmlScanResult {
    let mut out = XmlScanResult::default();
    if input.is_empty() {
        return out;
    }

    let ac = matcher();
    for m in ac.find_iter(input.as_bytes()) {
        let idx = m.pattern().as_usize();
        let (name, _pat) = PATTERNS[idx];
        out.matches.push(name.to_string());

        match name {
            "doctype" => out.has_doctype = true,
            "entity" => out.has_entity = true,
            "dtd_system" | "dtd_public" => out.has_doctype = true,
            "http" | "https" => out.has_external_ref = true,
            "href" | "xlink_href" | "src" => {
                // only count as external ref if url-ish tokens also appear
                // (keeps false positives lower).
            }
            "script_tag" | "onload" | "onerror" | "javascript" => out.has_scriptish = true,
            _ => {}
        }
    }

    // Deduplicate match names.
    out.matches.sort();
    out.matches.dedup();

    // Severity heuristic.
    // (Sandboxing + rlimits remain the real safety boundary.)
    let mut sev: u8 = 0;
    if out.has_entity {
        sev = sev.saturating_add(3);
    }
    if out.has_doctype {
        sev = sev.saturating_add(2);
    }
    if out.has_scriptish {
        sev = sev.saturating_add(2);
    }
    if out.has_external_ref {
        sev = sev.saturating_add(1);
    }
    out.severity = sev;

    out
}
