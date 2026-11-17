# Module `docversion_core::shortcut`

[← Workspace Index](../README.md) · [Crate `docversion_core`](../docversion_core.md)

Clean Markdown exporter and rich-text HTML converter for Shortcut Docs compatibility. Strips raw HTML tags outside fenced code blocks, normalizes internal Rustdoc/site links to clean relative Markdown paths, and converts sanitized Markdown (including GFM pipe tables and fenced code blocks) to semantic HTML with language-agnostic syntax highlighting for RTF paste.

## `fn sanitize_for_shortcut_docs`

Sanitizes generated Markdown for zero-friction import into Shortcut Docs. Preserves fenced code blocks (` ``` ` and `~~~`) and inline backtick spans verbatim. Converts `<br>` / `<br/>` into Markdown hard line breaks (`  \n`). Converts `<details>` / `<summary>` blocks into clean Markdown bold headers. Strips all other raw HTML tags (`<div>`, `<span>`, `<a name="...">`, etc.) while keeping inner text. Rewrites `.html` and root-prefixed (`/latest/...`, `/vX.Y.Z/...`) links to relative `.md` paths.

~~~rust
pub fn sanitize_for_shortcut_docs(markdown: &str) -> String
~~~

## `fn markdown_to_clean_html`

Converts sanitized Shortcut Markdown into compact, well-spaced semantic HTML with GFM table support and language-agnostic syntax highlighting for RTF conversion.

~~~rust
pub fn markdown_to_clean_html(markdown: &str) -> String
~~~

## `fn highlight_code_universal`

Language-agnostic lexical syntax highlighter emitting uniform styled spans so macOS `textutil` RTF conversion preserves consistent weight and monospace runs.

~~~rust
pub fn highlight_code_universal(line: &str) -> String
~~~

