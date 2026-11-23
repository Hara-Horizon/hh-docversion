use quote::ToTokens;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{ExprCall, ExprMacro, ExprMethodCall, ItemEnum, ItemFn, ItemStruct, ItemTrait, TypePath};

#[derive(Debug, Clone, Deserialize)]
pub struct BacklogTicket {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuditLogEntry {
    pub ticket_id: Option<String>,
    pub status: Option<String>,
    pub repo: Option<String>,
    pub commit_before: Option<String>,
    pub commit_after: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TicketContract {
    pub ticket_id: String,
    pub target_repo: Option<String>,
    pub required_symbols: Vec<String>,
    pub required_files: Vec<String>,
    pub required_tests: Vec<String>,
    pub required_neuromorphic_events: Vec<String>,
    pub required_agent_telemetry: Vec<String>,
    pub forbid_floats: bool,
    pub forbid_stubs: bool,
    pub forbid_blocking_io: bool,
    pub require_glial_pruning: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GapFinding {
    pub ticket_id: String,
    pub severity: &'static str, // "CRITICAL", "FAIL", "WARN"
    pub code: &'static str,
    pub location: String,
    pub detail: String,
}

/// Parses declarative contract tags—including universal repository/symbol contracts
/// and biomimetic/agentic invariants (`@requires-neuromorphic-event:`, `@forbid-blocking-io`,
/// `@requires-glial-pruning`, `@requires-agent-telemetry:`)—from ticket descriptions or TASKS.md.
pub fn parse_contract_from_text(ticket_id: &str, text: &str) -> Option<TicketContract> {
    let mut contract = TicketContract {
        ticket_id: ticket_id.to_string(),
        target_repo: None,
        required_symbols: Vec::new(),
        required_files: Vec::new(),
        required_tests: Vec::new(),
        required_neuromorphic_events: Vec::new(),
        required_agent_telemetry: Vec::new(),
        forbid_floats: false,
        forbid_stubs: false,
        forbid_blocking_io: false,
        require_glial_pruning: false,
    };

    let mut has_any_tag = false;

    for raw_line in text.lines() {
        let line = raw_line
            .trim()
            .trim_start_matches('-')
            .trim_start_matches('*')
            .trim();

        if let Some(val) = line.strip_prefix("@target-repo:") {
            let repo = val.trim().trim_matches('`');
            if !repo.is_empty() {
                contract.target_repo = Some(repo.to_string());
                has_any_tag = true;
            }
        } else if let Some(val) = line.strip_prefix("@requires-symbol:") {
            for sym in val.split(',') {
                let s = sym.trim().trim_matches('`');
                if !s.is_empty() {
                    contract.required_symbols.push(s.to_string());
                    has_any_tag = true;
                }
            }
        } else if let Some(val) = line.strip_prefix("@requires-file:") {
            for file in val.split(',') {
                let f = file.trim().trim_matches('`');
                if !f.is_empty() {
                    contract.required_files.push(f.to_string());
                    has_any_tag = true;
                }
            }
        } else if let Some(val) = line.strip_prefix("@requires-test:") {
            for test in val.split(',') {
                let t = test.trim().trim_matches('`');
                if !t.is_empty() {
                    contract.required_tests.push(t.to_string());
                    has_any_tag = true;
                }
            }
        } else if let Some(val) = line.strip_prefix("@requires-neuromorphic-event:") {
            for ev in val.split(',') {
                let e = ev.trim().trim_matches('`');
                if !e.is_empty() {
                    contract.required_neuromorphic_events.push(e.to_string());
                    has_any_tag = true;
                }
            }
        } else if let Some(val) = line.strip_prefix("@requires-agent-telemetry:") {
            for metric in val.split(',') {
                let m = metric.trim().trim_matches('`');
                if !m.is_empty() {
                    contract.required_agent_telemetry.push(m.to_string());
                    has_any_tag = true;
                }
            }
        } else if line.starts_with("@forbid-floats") {
            contract.forbid_floats = true;
            has_any_tag = true;
        } else if line.starts_with("@forbid-stubs") {
            contract.forbid_stubs = true;
            has_any_tag = true;
        } else if line.starts_with("@forbid-blocking-io") {
            contract.forbid_blocking_io = true;
            has_any_tag = true;
        } else if line.starts_with("@requires-glial-pruning") {
            contract.require_glial_pruning = true;
            has_any_tag = true;
        }
    }

    if has_any_tag {
        Some(contract)
    } else {
        None
    }
}

/// Loads ticket contracts from either a Shortcut JSON export (`[{id, name, description}]`)
/// or a Markdown spec (`TASKS.md`) containing `## <TICKET-ID>` headers.
pub fn load_contracts(manifest_path: &Path) -> Result<Vec<TicketContract>, String> {
    let raw = fs::read_to_string(manifest_path)
        .map_err(|e| format!("Failed to read manifest {}: {}", manifest_path.display(), e))?;

    if manifest_path.extension().and_then(|s| s.to_str()) == Some("json") {
        let tickets: Vec<BacklogTicket> = serde_json::from_str(&raw)
            .map_err(|e| format!("Invalid JSON backlog manifest: {}", e))?;
        Ok(tickets
            .into_iter()
            .filter_map(|t| parse_contract_from_text(&t.id, &t.description))
            .collect())
    } else {
        let mut contracts = Vec::new();
        let mut current_id: Option<String> = None;
        let mut buffer = String::new();

        for line in raw.lines() {
            if let Some(header) = line.strip_prefix("## ") {
                if let Some(id) = current_id.take() {
                    if let Some(c) = parse_contract_from_text(&id, &buffer) {
                        contracts.push(c);
                    }
                    buffer.clear();
                }
                let id_token = header
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(':');
                if !id_token.is_empty() {
                    current_id = Some(id_token.to_string());
                }
            } else if current_id.is_some() {
                buffer.push_str(line);
                buffer.push('\n');
            }
        }
        if let Some(id) = current_id {
            if let Some(c) = parse_contract_from_text(&id, &buffer) {
                contracts.push(c);
            }
        }
        Ok(contracts)
    }
}

/// Parses `audit.ndjson` to identify which tickets the local agent claimed as completed.
pub fn load_claimed_tickets(audit_log_path: &Path) -> BTreeMap<String, AuditLogEntry> {
    let mut claims = BTreeMap::new();
    let Ok(content) = fs::read_to_string(audit_log_path) else {
        return claims;
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<AuditLogEntry>(trimmed) {
            if let Some(ref tid) = entry.ticket_id {
                let is_done = entry
                    .status
                    .as_deref()
                    .map(|s| matches!(s.to_uppercase().as_str(), "DONE" | "COMPLETED" | "CLAIMED"))
                    .unwrap_or(true);
                if is_done {
                    claims.insert(tid.clone(), entry);
                }
            }
        }
    }
    claims
}

/// Parses `cargo test --message-format=json` output for passing test names.
pub fn load_passing_tests(test_receipt_path: Option<&Path>) -> BTreeSet<String> {
    let mut passed = BTreeSet::new();
    let Some(path) = test_receipt_path else {
        return passed;
    };
    let Ok(content) = fs::read_to_string(path) else {
        return passed;
    };

    for line in content.lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val.get("type").and_then(|v| v.as_str()) == Some("test")
                && val.get("event").and_then(|v| v.as_str()) == Some("ok")
            {
                if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
                    passed.insert(name.to_string());
                }
            }
        }
    }
    passed
}

#[derive(Default)]
struct AstAuditVisitor {
    pub public_symbols: BTreeSet<String>,
    pub all_identifiers: BTreeSet<String>,
    pub stub_findings: Vec<String>,
    pub float_usages: Vec<String>,
    pub blocking_io_usages: Vec<String>,
    pub has_glial_pruning_hook: bool,
    pub string_literals: BTreeSet<String>,
    in_test_mod: bool,
}

impl AstAuditVisitor {
    fn record_ident(&mut self, ident: &str) {
        self.all_identifiers.insert(ident.to_string());
        let lower = ident.to_ascii_lowercase();
        if lower.contains("prune")
            || lower.contains("microglia")
            || lower.contains("evict")
            || lower.contains("decay")
        {
            self.has_glial_pruning_hook = true;
        }
    }

    fn check_blocking_path(&mut self, path_str: &str) {
        if self.in_test_mod {
            return;
        }
        let compact: String = path_str.chars().filter(|c| !c.is_whitespace()).collect();
        const FORBIDDEN_BLOCKING: &[&str] = &[
            "std::fs",
            "std::net",
            "std::thread::sleep",
            "std::sync::Mutex",
            "std::sync::RwLock",
        ];
        for needle in FORBIDDEN_BLOCKING {
            if compact.contains(needle) {
                self.blocking_io_usages.push(format!(
                    "Blocking primitive `{}` detected in non-test reactive path",
                    compact
                ));
                break;
            }
        }
    }
}

impl<'ast> Visit<'ast> for AstAuditVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let is_test = node.attrs.iter().any(|attr| {
            attr.path().is_ident("cfg")
                && attr.meta.to_token_stream().to_string().contains("test")
        });
        let prev = self.in_test_mod;
        if is_test {
            self.in_test_mod = true;
        }
        visit::visit_item_mod(self, node);
        self.in_test_mod = prev;
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let is_test_fn = node.attrs.iter().any(|a| a.path().is_ident("test"));
        let name = node.sig.ident.to_string();
        if matches!(node.vis, syn::Visibility::Public(_)) {
            self.public_symbols.insert(name.clone());
        }
        if !self.in_test_mod && !is_test_fn {
            self.record_ident(&name);
        }
        let prev = self.in_test_mod;
        if is_test_fn {
            self.in_test_mod = true;
        }
        visit::visit_item_fn(self, node);
        self.in_test_mod = prev;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let name = node.sig.ident.to_string();
        if matches!(node.vis, syn::Visibility::Public(_)) {
            self.public_symbols.insert(name.clone());
        }
        if !self.in_test_mod {
            self.record_ident(&name);
        }
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let name = node.ident.to_string();
        if matches!(node.vis, syn::Visibility::Public(_)) {
            self.public_symbols.insert(name.clone());
        }
        if !self.in_test_mod {
            self.record_ident(&name);
            if name.starts_with("Mock") {
                self.stub_findings
                    .push(format!("Production Mock struct `{}` found outside #[cfg(test)]", name));
            }
        }
        visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast ItemEnum) {
        let name = node.ident.to_string();
        if matches!(node.vis, syn::Visibility::Public(_)) {
            self.public_symbols.insert(name.clone());
        }
        if !self.in_test_mod {
            self.record_ident(&name);
            for variant in &node.variants {
                self.record_ident(&variant.ident.to_string());
            }
        }
        visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast ItemTrait) {
        let name = node.ident.to_string();
        if matches!(node.vis, syn::Visibility::Public(_)) {
            self.public_symbols.insert(name.clone());
        }
        if !self.in_test_mod {
            self.record_ident(&name);
        }
        visit::visit_item_trait(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if !self.in_test_mod {
            let path_str = node.func.to_token_stream().to_string();
            self.check_blocking_path(&path_str);
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if !self.in_test_mod {
            let method = node.method.to_string();
            self.record_ident(&method);
            if method == "block_on" {
                self.blocking_io_usages
                    .push("Blocking `.block_on()` call detected in non-test reactive path".into());
            }
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast ExprMacro) {
        if !self.in_test_mod {
            if let Some(ident) = node.mac.path.get_ident() {
                let name = ident.to_string();
                if name == "todo" || name == "unimplemented" {
                    self.stub_findings
                        .push(format!("`{}!()` stub macro left in production code", name));
                }
            }
        }
        visit::visit_expr_macro(self, node);
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        if !self.in_test_mod {
            self.string_literals.insert(node.value());
        }
        visit::visit_lit_str(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast TypePath) {
        if !self.in_test_mod {
            let full_path = node.path.to_token_stream().to_string();
            self.check_blocking_path(&full_path);

            if let Some(ident) = node.path.get_ident() {
                let ty = ident.to_string();
                self.record_ident(&ty);
                if ty == "f32" || ty == "f64" {
                    self.float_usages
                        .push(format!("Floating-point type `{}` found in non-test module", ty));
                }
            }
        }
        visit::visit_type_path(self, node);
    }
}

/// Checks via `gix` whether the target repository is a valid Git worktree and whether
/// tree changes occurred between `commit_before` and `commit_after` (or working tree dirty).
fn verify_git_activity(repo_path: &Path, claim: Option<&AuditLogEntry>) -> Result<bool, String> {
    let repo = gix::open(repo_path)
        .map_err(|e| format!("Failed to open git repo at {}: {}", repo_path.display(), e))?;

    let _work_dir = repo
        .work_dir()
        .ok_or_else(|| format!("Repository {} is bare (no work_dir)", repo_path.display()))?;

    if let Some(entry) = claim {
        if let (Some(before), Some(after)) = (&entry.commit_before, &entry.commit_after) {
            if before == after {
                let is_dirty = repo.is_dirty().unwrap_or(false);
                return Ok(is_dirty);
            }
        }
    }

    Ok(true)
}

pub fn audit_ticket(
    workspace_root: &Path,
    contract: &TicketContract,
    claim: Option<&AuditLogEntry>,
    passing_tests: &BTreeSet<String>,
) -> Vec<GapFinding> {
    let mut findings = Vec::new();

    let Some(ref target_repo) = contract.target_repo else {
        findings.push(GapFinding {
            ticket_id: contract.ticket_id.clone(),
            severity: "WARN",
            code: "MISSING_TARGET_REPO_TAG",
            location: "manifest".into(),
            detail: "Ticket contract has no `@target-repo:` tag; skipped repo inspection.".into(),
        });
        return findings;
    };

    let repo_path = workspace_root.join(target_repo);
    if !repo_path.exists() {
        findings.push(GapFinding {
            ticket_id: contract.ticket_id.clone(),
            severity: "CRITICAL",
            code: "PHANTOM_CLAIM",
            location: repo_path.display().to_string(),
            detail: format!("Target repository `{}` does not exist on disk.", target_repo),
        });
        return findings;
    }

    // 1. Git (`gix`) ground-truth activity check
    match verify_git_activity(&repo_path, claim) {
        Ok(false) => findings.push(GapFinding {
            ticket_id: contract.ticket_id.clone(),
            severity: "CRITICAL",
            code: "PHANTOM_CLAIM",
            location: target_repo.clone(),
            detail: "Agent logged completion in audit.ndjson, but gix shows zero commit or worktree changes.".into(),
        }),
        Err(err) => findings.push(GapFinding {
            ticket_id: contract.ticket_id.clone(),
            severity: "WARN",
            code: "GIT_INSPECTION_ERROR",
            location: target_repo.clone(),
            detail: err,
        }),
        Ok(true) => {}
    }

    // 2. Required file checks
    for rel_file in &contract.required_files {
        let full_path = repo_path.join(rel_file);
        match fs::metadata(&full_path) {
            Ok(meta) if meta.len() > 0 => {}
            Ok(_) => findings.push(GapFinding {
                ticket_id: contract.ticket_id.clone(),
                severity: "FAIL",
                code: "EMPTY_FILE",
                location: rel_file.clone(),
                detail: "Required file exists but is 0 bytes.".into(),
            }),
            Err(_) => findings.push(GapFinding {
                ticket_id: contract.ticket_id.clone(),
                severity: "FAIL",
                code: "MISSING_FILE",
                location: rel_file.clone(),
                detail: "Required file declared via `@requires-file:` is missing.".into(),
            }),
        }
    }

    // 3. Walk Rust source files and inspect AST via `syn`
    let mut visitor = AstAuditVisitor::default();
    walk_rs_files(&repo_path, &mut |file_path| {
        if file_path.components().any(|c| c.as_os_str() == "target") {
            return;
        }
        if let Ok(content) = fs::read_to_string(file_path) {
            if let Ok(syntax) = syn::parse_file(&content) {
                let prev_stubs = visitor.stub_findings.len();
                let prev_floats = visitor.float_usages.len();
                let prev_blocking = visitor.blocking_io_usages.len();
                visitor.visit_file(&syntax);

                if contract.forbid_stubs {
                    for stub in &visitor.stub_findings[prev_stubs..] {
                        findings.push(GapFinding {
                            ticket_id: contract.ticket_id.clone(),
                            severity: "FAIL",
                            code: "STUB_IMPLEMENTATION",
                            location: file_path.display().to_string(),
                            detail: stub.clone(),
                        });
                    }
                }
                if contract.forbid_floats {
                    for flt in &visitor.float_usages[prev_floats..] {
                        findings.push(GapFinding {
                            ticket_id: contract.ticket_id.clone(),
                            severity: "CRITICAL",
                            code: "INVARIANT_BREACH",
                            location: file_path.display().to_string(),
                            detail: flt.clone(),
                        });
                    }
                }
                if contract.forbid_blocking_io {
                    for blk in &visitor.blocking_io_usages[prev_blocking..] {
                        findings.push(GapFinding {
                            ticket_id: contract.ticket_id.clone(),
                            severity: "FAIL",
                            code: "BLOCKING_IO_IN_REACTIVE_LOOP",
                            location: file_path.display().to_string(),
                            detail: blk.clone(),
                        });
                    }
                }
            }
        }
    });

    // 4. Verify `@requires-symbol:` contracts
    for sym in &contract.required_symbols {
        if !visitor.public_symbols.contains(sym) {
            findings.push(GapFinding {
                ticket_id: contract.ticket_id.clone(),
                severity: "FAIL",
                code: "MISSING_CONTRACT_SYMBOL",
                location: target_repo.clone(),
                detail: format!(
                    "Public symbol `{}` declared via `@requires-symbol:` is absent from AST.",
                    sym
                ),
            });
        }
    }

    // 5. Verify `@requires-neuromorphic-event:` handlers
    for ev in &contract.required_neuromorphic_events {
        let found = visitor.all_identifiers.iter().any(|id| id.contains(ev));
        if !found {
            findings.push(GapFinding {
                ticket_id: contract.ticket_id.clone(),
                severity: "FAIL",
                code: "MISSING_NEUROMORPHIC_HANDLER",
                location: target_repo.clone(),
                detail: format!(
                    "Neuromorphic event `{}` declared via `@requires-neuromorphic-event:` has no AST handler.",
                    ev
                ),
            });
        }
    }

    // 6. Verify `@requires-glial-pruning` bounded memory/context eviction hooks
    if contract.require_glial_pruning && !visitor.has_glial_pruning_hook {
        findings.push(GapFinding {
            ticket_id: contract.ticket_id.clone(),
            severity: "CRITICAL",
            code: "GLIAL_MEMORY_LEAK_RISK",
            location: target_repo.clone(),
            detail: "Contract requires `@requires-glial-pruning`, but no bounded decay/eviction/pruning symbol was found in non-test AST.".into(),
        });
    }

    // 7. Verify `@requires-agent-telemetry:` structured trace/metric identifiers
    for metric in &contract.required_agent_telemetry {
        let in_ident = visitor.all_identifiers.iter().any(|id| id.contains(metric));
        let in_lit = visitor.string_literals.iter().any(|s| s.contains(metric));
        if !in_ident && !in_lit {
            findings.push(GapFinding {
                ticket_id: contract.ticket_id.clone(),
                severity: "WARN",
                code: "MISSING_AGENT_TELEMETRY",
                location: target_repo.clone(),
                detail: format!(
                    "Declared `@requires-agent-telemetry:` metric `{}` is absent from non-test execution paths.",
                    metric
                ),
            });
        }
    }

    // 8. Verify `@requires-test:` execution receipts
    for req_test in &contract.required_tests {
        let matched = passing_tests.iter().any(|t| t.contains(req_test));
        if !matched {
            findings.push(GapFinding {
                ticket_id: contract.ticket_id.clone(),
                severity: "WARN",
                code: "UNVERIFIED_EXECUTION",
                location: target_repo.clone(),
                detail: format!(
                    "No passing `cargo test` JSON receipt found matching `{}`.",
                    req_test
                ),
            });
        }
    }

    findings
}

pub fn render_gap_reports(findings: &[GapFinding]) -> (String, String) {
    let mut md = String::new();
    let mut llms = String::new();

    writeln!(md, "# AUDIT_GAP_REPORT — Claim vs. Ground-Truth Verification\n").unwrap();
    writeln!(llms, "# llms-gaps.txt — Unimplemented & Failed Contract Deliverables\n").unwrap();

    if findings.is_empty() {
        writeln!(md, "All claimed ticket contracts passed AST, Git (`gix`), and test receipt checks.").unwrap();
        writeln!(llms, "STATUS: CLEAN (0 gaps)").unwrap();
        return (md, llms);
    }

    writeln!(md, "| Ticket | Severity | Code | Location | Detail |").unwrap();
    writeln!(md, "| :--- | :--- | :--- | :--- | :--- |").unwrap();

    for f in findings {
        writeln!(
            md,
            "| `{}` | **{}** | `{}` | `{}` | {} |",
            f.ticket_id, f.severity, f.code, f.location, f.detail
        )
        .unwrap();

        writeln!(
            llms,
            "[{}][{}] {} @ {} -> {}",
            f.ticket_id, f.severity, f.code, f.location, f.detail
        )
        .unwrap();
    }

    (md, llms)
}

fn walk_rs_files(dir: &Path, cb: &mut dyn FnMut(&Path)) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path: PathBuf = entry.path();
            if path.is_dir() {
                walk_rs_files(&path, cb);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                cb(&path);
            }
        }
    }
}