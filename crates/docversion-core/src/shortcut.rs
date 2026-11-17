//! Clean Markdown exporter and rich-text HTML converter for Shortcut Docs compatibility.
//!
//! Strips raw HTML tags outside fenced code blocks, normalizes internal
//! Rustdoc/site links to clean relative Markdown paths, and converts
//! sanitized Markdown (including GFM pipe tables and fenced code blocks)
//! to semantic HTML with language-agnostic syntax highlighting for RTF paste.

const PRE_STYLE: &str = "font-family: 'SF Mono', Menlo, Monaco, Consolas, monospace; font-size: 12.5px; line-height: 1.5; background-color: #f6f8fa; color: #1f2328; border: 1px solid #d0d7de; padding: 10px 14px; white-space: pre-wrap;";
const INLINE_CODE_STYLE: &str = "font-family: 'SF Mono', Menlo, Monaco, Consolas, monospace; font-size: 12.5px; background-color: #eff1f3; color: #0550ae;";
const TABLE_STYLE: &str = "border-collapse: collapse; width: 100%; font-size: 13px;";
const TH_STYLE: &str = "border: 1px solid #d0d7de; background-color: #f6f8fa; color: #0f141c; font-weight: 600; text-align: left; padding: 6px 10px;";
const TD_STYLE: &str = "border: 1px solid #d0d7de; color: #1f2328; padding: 6px 10px; vertical-align: top;";

/// Sanitizes generated Markdown for zero-friction import into Shortcut Docs.
///
/// - Preserves fenced code blocks (` ``` ` and `~~~`) and inline backtick spans verbatim.
/// - Converts `<br>` / `<br/>` into Markdown hard line breaks (`  \n`).
/// - Converts `<details>` / `<summary>` blocks into clean Markdown bold headers.
/// - Strips all other raw HTML tags (`<div>`, `<span>`, `<a name="...">`, etc.) while keeping inner text.
/// - Rewrites `.html` and root-prefixed (`/latest/...`, `/vX.Y.Z/...`) links to relative `.md` paths.
pub fn sanitize_for_shortcut_docs(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut in_fence = false;
    let mut fence_marker: Option<&str> = None;

    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();

        if let Some(marker) = detect_fence_prefix(trimmed) {
            if !in_fence {
                in_fence = true;
                fence_marker = Some(marker);
            } else if fence_marker == Some(marker) {
                in_fence = false;
                fence_marker = None;
            }
            output.push_str(line);
            continue;
        }

        if in_fence {
            output.push_str(line);
            continue;
        }

        let processed = process_prose_line(line);
        output.push_str(&processed);
    }

    collapse_excess_blank_lines(&output)
}

/// Converts sanitized Shortcut Markdown into compact, well-spaced semantic HTML with
/// GFM table support and language-agnostic syntax highlighting for RTF conversion.
pub fn markdown_to_clean_html(markdown: &str) -> String {
    let mut html = String::with_capacity(markdown.len() * 2 + 1024);
    html.push_str(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  body {
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
    font-size: 14px;
    line-height: 1.55;
    color: #1f2328;
  }
  h1 {
    font-size: 24px;
    font-weight: 700;
    color: #0f141c;
  }
  h2 {
    font-size: 19px;
    font-weight: 600;
    color: #0f141c;
  }
  h3 {
    font-size: 16px;
    font-weight: 600;
    color: #1f2328;
  }
  h4, h5, h6 {
    font-size: 14px;
    font-weight: 600;
    color: #424a53;
  }
  blockquote {
    padding: 8px 14px;
    border-left: 3px solid #d97706;
    background-color: #f6f8fa;
    color: #424a53;
  }
  li {
    line-height: 1.55;
  }
  .spacer {
    font-size: 6px;
    line-height: 6px;
  }
</style>
</head>
<body>
"#,
    );

    let spacer = "<p class=\"spacer\">&nbsp;</p>\n";
    let lines: Vec<&str> = markdown.lines().collect();
    let mut idx = 0;
    let mut in_fence = false;
    let mut fence_marker: Option<&str> = None;
    let mut in_list = false;
    let mut has_emitted_block = false;
    let mut last_was_spacer = false;

    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim_start();

        if let Some(marker) = detect_fence_prefix(trimmed) {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            if !in_fence {
                in_fence = true;
                fence_marker = Some(marker);
                html.push_str(&format!("<pre style=\"{PRE_STYLE}\"><code>"));
            } else if fence_marker == Some(marker) {
                in_fence = false;
                fence_marker = None;
                html.push_str("</code></pre>\n");
                has_emitted_block = true;
                last_was_spacer = false;
            } else {
                html.push_str(&highlight_code_universal(line));
                html.push('\n');
            }
            idx += 1;
            continue;
        }

        if in_fence {
            html.push_str(&highlight_code_universal(line));
            html.push('\n');
            idx += 1;
            continue;
        }

        if trimmed.is_empty() {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            idx += 1;
            continue;
        }

        // GFM Pipe Table (`| H1 | H2 |` followed by `| --- | --- |`)
        if is_pipe_table_row(trimmed)
            && idx + 1 < lines.len()
            && is_pipe_table_separator(lines[idx + 1].trim())
        {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            let headers = parse_pipe_cells(trimmed);
            html.push_str(&format!("<table style=\"{TABLE_STYLE}\">\n<thead><tr>"));
            for h in headers {
                let cell_html = render_inline_html(&h);
                html.push_str(&format!("<th style=\"{TH_STYLE}\">{cell_html}</th>"));
            }
            html.push_str("</tr></thead>\n<tbody>\n");
            idx += 2; // skip header + separator

            while idx < lines.len() && is_pipe_table_row(lines[idx].trim()) {
                let cells = parse_pipe_cells(lines[idx].trim());
                html.push_str("<tr>");
                for c in cells {
                    let cell_html = render_inline_html(&c);
                    html.push_str(&format!("<td style=\"{TD_STYLE}\">{cell_html}</td>"));
                }
                html.push_str("</tr>\n");
                idx += 1;
            }
            html.push_str("</tbody>\n</table>\n");
            has_emitted_block = true;
            last_was_spacer = false;
            continue;
        }

        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            html.push_str("<hr/>\n");
            has_emitted_block = true;
            last_was_spacer = false;
            idx += 1;
            continue;
        }

        // Headings (# through ######): emit at most one spacer immediately before h1-h3
        let hash_count = trimmed.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&hash_count) && trimmed.as_bytes().get(hash_count) == Some(&b' ') {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            if has_emitted_block && hash_count <= 3 && !last_was_spacer {
                html.push_str(spacer);
            }
            let text = render_inline_html(trimmed[hash_count + 1..].trim());
            html.push_str(&format!("<h{hash_count}>{text}</h{hash_count}>\n"));
            has_emitted_block = true;
            last_was_spacer = false;
            idx += 1;
            continue;
        }

        // Blockquotes (> ...)
        if let Some(quote_body) = trimmed.strip_prefix("> ") {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            let text = render_inline_html(quote_body);
            html.push_str(&format!("<blockquote><p>{text}</p></blockquote>\n"));
            has_emitted_block = true;
            last_was_spacer = false;
            idx += 1;
            continue;
        }

        // Unordered list items (* ... or - ...)
        if let Some(item_body) = trimmed
            .strip_prefix("* ")
            .or_else(|| trimmed.strip_prefix("- "))
        {
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            let text = render_inline_html(item_body);
            html.push_str(&format!("<li>{text}</li>\n"));
            has_emitted_block = true;
            last_was_spacer = false;
            idx += 1;
            continue;
        }

        if in_list {
            html.push_str("</ul>\n");
            in_list = false;
        }

        let text = render_inline_html(trimmed);
        html.push_str(&format!("<p>{text}</p>\n"));
        has_emitted_block = true;
        last_was_spacer = false;
        idx += 1;
    }

    if in_list {
        html.push_str("</ul>\n");
    }
    if in_fence {
        html.push_str("</code></pre>\n");
    }

    html.push_str("</body></html>\n");
    html
}

fn is_pipe_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() >= 3
}

fn is_pipe_table_separator(line: &str) -> bool {
    if !is_pipe_table_row(line) {
        return false;
    }
    let cells = parse_pipe_cells(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let s = c.trim();
            !s.is_empty() && s.chars().all(|ch| ch == '-' || ch == ':')
        })
}

fn parse_pipe_cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

const CODE_KW_STYLE: &str = "color: #cf222e; font-weight: 400;";
const CODE_TYPE_STYLE: &str = "color: #953800; font-weight: 400;";
const CODE_STR_STYLE: &str = "color: #0a3069; font-weight: 400;";
const CODE_NUM_STYLE: &str = "color: #0550ae; font-weight: 400;";
const CODE_CMT_STYLE: &str = "color: #6e7781; font-style: italic; font-weight: 400;";
const CODE_PLAIN_STYLE: &str = "color: #1f2328; font-weight: 400;";

fn span(style: &str, raw: &str) -> String {
    format!("<span style=\"{style}\">{}</span>", escape_html(raw))
}

/// Language-agnostic lexical syntax highlighter emitting uniform styled spans
/// so macOS `textutil` RTF conversion preserves consistent weight and monospace runs.
pub fn highlight_code_universal(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("--") {
        return span(CODE_CMT_STYLE, line);
    }

    let mut out = String::with_capacity(line.len() * 3);
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Inline // comment
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '/' {
            let rest: String = chars[i..].iter().collect();
            out.push_str(&span(CODE_CMT_STYLE, &rest));
            break;
        }

        // String literal ("..." or '...')
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            let start = i;
            i += 1;
            while i < len {
                if chars[i] == '\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let lit: String = chars[start..i].iter().collect();
            out.push_str(&span(CODE_STR_STYLE, &lit));
            continue;
        }

        // Numeric literal
        if chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_ascii_alphanumeric()) {
            let start = i;
            while i < len
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_')
            {
                i += 1;
            }
            let num: String = chars[start..i].iter().collect();
            out.push_str(&span(CODE_NUM_STYLE, &num));
            continue;
        }

        // Identifier / keyword / PascalCase type
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            if is_universal_keyword(&ident) {
                out.push_str(&span(CODE_KW_STYLE, &ident));
            } else if ident.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && ident.chars().any(|c| c.is_ascii_lowercase())
            {
                out.push_str(&span(CODE_TYPE_STYLE, &ident));
            } else {
                out.push_str(&span(CODE_PLAIN_STYLE, &ident));
            }
            continue;
        }

        // Group contiguous whitespace/punctuation into a single uniform plain span
        let start = i;
        while i < len
            && !chars[i].is_ascii_alphanumeric()
            && chars[i] != '_'
            && chars[i] != '"'
            && chars[i] != '\''
            && !(chars[i] == '/' && i + 1 < len && chars[i + 1] == '/')
        {
            i += 1;
        }
        let punct: String = chars[start..i].iter().collect();
        out.push_str(&span(CODE_PLAIN_STYLE, &punct));
    }

    out
}

fn is_universal_keyword(word: &str) -> bool {
    matches!(
        word,
        "pub" | "fn" | "struct" | "enum" | "trait" | "impl" | "mod" | "use" | "let" | "mut"
            | "const" | "static" | "match" | "if" | "else" | "for" | "while" | "loop" | "return"
            | "async" | "await" | "where" | "type" | "self" | "Self" | "crate" | "super"
            | "function" | "export" | "import" | "interface" | "class" | "extends" | "implements"
            | "def" | "from" | "as" | "None" | "True" | "False" | "true" | "false" | "null"
            | "func" | "package" | "var" | "defer" | "SELECT" | "FROM" | "WHERE" | "CREATE"
            | "TABLE" | "PRIMARY" | "KEY" | "NOT" | "NULL"
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_inline_html(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    let mut buf = String::new();

    for ch in line.chars() {
        if ch == '`' {
            if in_code {
                out.push_str(&format!("<code style=\"{INLINE_CODE_STYLE}\">"));
                out.push_str(&escape_html(&buf));
                out.push_str("</code>");
                buf.clear();
                in_code = false;
            } else {
                out.push_str(&apply_bold_html(&escape_html(&buf)));
                buf.clear();
                in_code = true;
            }
        } else {
            buf.push(ch);
        }
    }

    if !buf.is_empty() {
        if in_code {
            out.push_str(&format!("<code style=\"{INLINE_CODE_STYLE}\">"));
            out.push_str(&escape_html(&buf));
            out.push_str("</code>");
        } else {
            out.push_str(&apply_bold_html(&escape_html(&buf)));
        }
    }

    out
}

fn apply_bold_html(escaped: &str) -> String {
    let mut out = String::with_capacity(escaped.len());
    let mut rest = escaped;
    while let Some(start) = rest.find("**") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        if let Some(end) = after_start.find("**") {
            out.push_str("<strong>");
            out.push_str(&after_start[..end]);
            out.push_str("</strong>");
            rest = &after_start[end + 2..];
        } else {
            out.push_str("**");
            rest = after_start;
            break;
        }
    }
    out.push_str(rest);
    out
}

fn detect_fence_prefix(trimmed_line: &str) -> Option<&'static str> {
    if trimmed_line.starts_with("```") {
        Some("```")
    } else if trimmed_line.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

/// Processes a single non-fenced Markdown line, respecting inline code spans (`...`).
fn process_prose_line(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut chars = line.char_indices().peekable();
    let mut in_inline_code = false;
    let mut inline_ticks = 0usize;
    let mut prose_buf = String::new();

    while let Some((_idx, ch)) = chars.next() {
        if ch == '`' {
            let mut tick_count = 1;
            while let Some(&(_, '`')) = chars.peek() {
                chars.next();
                tick_count += 1;
            }

            if !in_inline_code {
                if !prose_buf.is_empty() {
                    result.push_str(&transform_prose_segment(&prose_buf));
                    prose_buf.clear();
                }
                in_inline_code = true;
                inline_ticks = tick_count;
            } else if tick_count == inline_ticks {
                in_inline_code = false;
                inline_ticks = 0;
            }

            for _ in 0..tick_count {
                result.push('`');
            }
        } else if in_inline_code {
            result.push(ch);
        } else {
            prose_buf.push(ch);
        }
    }

    if !prose_buf.is_empty() {
        result.push_str(&transform_prose_segment(&prose_buf));
    }

    result
}

/// Strips HTML tags and rewrites links inside a pure-prose segment (outside code spans).
fn transform_prose_segment(segment: &str) -> String {
    let no_html = strip_html_tags(segment);
    rewrite_markdown_links(&no_html)
}

/// Strips raw HTML tags while converting structural tags (`<br>`, `<summary>`) to Markdown.
fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(&next) = bytes.get(i + 1) {
                let is_tag_start =
                    next == b'/' || next == b'!' || (next as char).is_ascii_alphabetic();

                if is_tag_start {
                    if let Some(close_offset) = input[i..].find('>') {
                        let raw_tag = &input[i..=i + close_offset];
                        let replacement = map_structural_tag(raw_tag);
                        out.push_str(replacement);
                        i += close_offset + 1;
                        continue;
                    }
                }
            }
        }

        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

/// Maps select structural HTML tags to clean Markdown equivalents; drops all others.
fn map_structural_tag(raw_tag: &str) -> &'static str {
    let inner = raw_tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim();

    let tag_name = inner
        .trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    let is_closing = inner.starts_with('/');

    match tag_name.as_str() {
        "br" => "  \n",
        "hr" => "\n---\n",
        "summary" if !is_closing => "**",
        "summary" if is_closing => "**\n",
        _ => "",
    }
}

/// Rewrites `[label](target)` Markdown links to clean relative `.md` paths for Shortcut Docs.
fn rewrite_markdown_links(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(open_bracket) = rest.find('[') {
        out.push_str(&rest[..open_bracket]);
        let after_open = &rest[open_bracket..];

        if let Some(mid) = after_open.find("](") {
            let label = &after_open[1..mid];
            let after_mid = &after_open[mid + 2..];

            if let Some(close_paren) = after_mid.find(')') {
                let raw_target = after_mid[..close_paren].trim();
                let clean_target = normalize_target_path(raw_target);

                out.push('[');
                out.push_str(label);
                out.push_str("](");
                out.push_str(&clean_target);
                out.push(')');

                rest = &after_mid[close_paren + 1..];
                continue;
            }
        }

        out.push('[');
        rest = &after_open[1..];
    }

    out.push_str(rest);
    out
}

/// Normalizes internal doc targets into relative `.md` paths while preserving external URLs.
fn normalize_target_path(target: &str) -> String {
    if target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
    {
        return target.to_string();
    }

    if let Some(anchor) = target.strip_prefix('#') {
        return format!("#{}", slugify_anchor(anchor));
    }

    let (raw_path, fragment) = match target.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (target, None),
    };

    let stripped_prefix = strip_version_prefix(raw_path.trim_start_matches('/'));

    let relative_md = if stripped_prefix.ends_with("/index.html") {
        let dir = stripped_prefix.trim_end_matches("/index.html");
        format!("./{dir}.md")
    } else if stripped_prefix == "index.html" {
        "./index.md".to_string()
    } else if let Some(stem) = stripped_prefix.strip_suffix(".html") {
        if stem.starts_with("./") || stem.starts_with("../") {
            format!("{stem}.md")
        } else {
            format!("./{stem}.md")
        }
    } else if stripped_prefix.ends_with(".md") {
        if stripped_prefix.starts_with("./") || stripped_prefix.starts_with("../") {
            stripped_prefix.to_string()
        } else {
            format!("./{stripped_prefix}")
        }
    } else {
        stripped_prefix.to_string()
    };

    match fragment {
        Some(f) if !f.is_empty() => format!("{relative_md}#{}", slugify_anchor(f)),
        _ => relative_md,
    }
}

fn strip_version_prefix(path: &str) -> &str {
    if let Some(rest) = path.strip_prefix("latest/") {
        return rest;
    }
    if let Some(rest) = path.strip_prefix('v') {
        if let Some((ver, remainder)) = rest.split_once('/') {
            let is_semver = ver
                .split('.')
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));
            if is_semver {
                return remainder;
            }
        }
    }
    path
}

fn slugify_anchor(anchor: &str) -> String {
    anchor
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Prevents more than two consecutive newlines after stripping block-level HTML wrappers.
fn collapse_excess_blank_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut consecutive_newlines = 0;

    for ch in input.chars() {
        if ch == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                out.push(ch);
            }
        } else {
            consecutive_newlines = 0;
            out.push(ch);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_raw_html_outside_code_fences_only() {
        let input = concat!(
            "# `JournalBatch`\n\n",
            "<div class=\"topbar\">Version v0.1.0</div>\n",
            "<details><summary>Invariants</summary>\n",
            "Zero-sum check<br/>Must balance.\n",
            "</details>\n\n",
            "Inline code keeps `Result<T, E>` and `<span>raw</span>` untouched.\n\n",
            "~~~\n",
            "fn verify<T: Ord>(items: Vec<T>) -> Result<(), String> {\n",
            "    println!(\"<div>not stripped</div>\");\n",
            "    Ok(())\n",
            "}\n",
            "~~~\n"
        );

        let cleaned = sanitize_for_shortcut_docs(input);

        assert!(!cleaned.contains("<div class=\"topbar\">"));
        assert!(!cleaned.contains("<details>"));
        assert!(!cleaned.contains("<br/>"));
        assert!(cleaned.contains("**Invariants**\n"));
        assert!(cleaned.contains("Zero-sum check  \nMust balance."));
        assert!(cleaned.contains("`Result<T, E>`"));
        assert!(cleaned.contains("`<span>raw</span>`"));
        assert!(cleaned.contains("println!(\"<div>not stripped</div>\");"));
    }

    #[test]
    fn rewrites_rustdoc_and_versioned_links_to_relative_md() {
        let input = concat!(
            "See [Tuos Ledger](/latest/tuos/index.html) and ",
            "[JournalBatch](/v0.1.0/tuos/struct.JournalBatch.html#method.commit).\n",
            "External [GitHub](https://github.com/Hara-Horizon/hh-docversion) stays intact.\n",
            "Local anchor [Invariants](#System Invariants) is slugified.\n"
        );

        let cleaned = sanitize_for_shortcut_docs(input);

        assert!(cleaned.contains("[Tuos Ledger](./tuos.md)"));
        assert!(cleaned.contains("[JournalBatch](./tuos/struct.JournalBatch.md#method-commit)"));
        assert!(cleaned.contains("[GitHub](https://github.com/Hara-Horizon/hh-docversion)"));
        assert!(cleaned.contains("[Invariants](#system-invariants)"));
    }

    #[test]
    fn converts_markdown_tables_and_code_to_semantic_html() {
        let md = concat!(
            "# `hh-docversion`\n\n",
            "| Crate | Role |\n",
            "| :--- | :--- |\n",
            "| `docversion-core` | Core primitives |\n",
            "| `cargo-docversion` | CLI orchestrator |\n\n",
            "~~~rust\n",
            "pub fn check<T>(x: T) -> Result<(), String> {}\n",
            "~~~\n"
        );

        let html = markdown_to_clean_html(md);

        assert!(html.contains("<table style="));
        assert!(html.contains("<th style="));
        assert!(html.contains("Crate</th>"));
        assert!(html.contains("docversion-core</code></td>"));
        assert!(html.contains("<pre style="));
        assert!(html.contains("<span style=\"color: #cf222e; font-weight: 400;\">pub</span>"));
        assert_eq!(html.matches("<p class=\"spacer\">").count(), 0);
    }
}