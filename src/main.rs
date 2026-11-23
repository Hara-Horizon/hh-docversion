use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

// Ensure `pub mod audit;` is declared in src/lib.rs or src/main.rs
use hh_docversion::audit::{
    audit_ticket, load_claimed_tickets, load_contracts, load_passing_tests, render_gap_reports,
};

#[derive(Parser)]
#[command(name = "hh-docversion", version, about = "AST doc bundler & deterministic claim auditor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Audits local agent completion claims against Shortcut/Markdown contracts, AST, and gix diffs
    AuditClaims {
        /// Path to shortcut_backlog.json or TASKS.md containing declarative `@...` tags
        #[arg(long)]
        manifest: PathBuf,

        /// Path to cline-exec audit.ndjson log
        #[arg(long, default_value = "audit.ndjson")]
        audit_log: PathBuf,

        /// Root directory containing sibling repositories
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,

        /// Optional path to `cargo test --message-format=json` output file
        #[arg(long)]
        test_receipts: Option<PathBuf>,

        /// Audit all tickets marked DONE/CLAIMED in audit.ndjson
        #[arg(long)]
        all_claimed: bool,

        /// Optional comma-separated ticket IDs to audit explicitly
        #[arg(long, value_delimiter = ',')]
        tickets: Option<Vec<String>>,

        /// Output directory for AUDIT_GAP_REPORT.md and llms-gaps.txt
        #[arg(long, default_value = "target/doc_site")]
        out_dir: PathBuf,

        /// Exit with non-zero code if any CRITICAL or FAIL gap is detected
        #[arg(long)]
        fail_on_gap: bool,
    },
    // ... keep your existing Bundle / Generate subcommands here ...
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::AuditClaims {
            manifest,
            audit_log,
            workspace_root,
            test_receipts,
            all_claimed,
            tickets,
            out_dir,
            fail_on_gap,
        } => {
            let contracts = match load_contracts(&manifest) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error loading manifest: {}", e);
                    return ExitCode::from(2);
                }
            };

            let claims = load_claimed_tickets(&audit_log);
            let passing_tests = load_passing_tests(test_receipts.as_deref());

            let target_ids: Option<Vec<String>> = if let Some(explicit) = tickets {
                Some(explicit)
            } else if all_claimed {
                Some(claims.keys().cloned().collect())
            } else {
                None
            };

            let mut all_findings = Vec::new();
            for contract in &contracts {
                if let Some(ref filter) = target_ids {
                    if !filter.contains(&contract.ticket_id) {
                        continue;
                    }
                }
                let claim_entry = claims.get(&contract.ticket_id);
                let mut findings =
                    audit_ticket(&workspace_root, contract, claim_entry, &passing_tests);
                all_findings.append(&mut findings);
            }

            let (md_report, llms_gaps) = render_gap_reports(&all_findings);
            let _ = std::fs::create_dir_all(&out_dir);
            let md_path = out_dir.join("AUDIT_GAP_REPORT.md");
            let llms_path = out_dir.join("llms-gaps.txt");

            std::fs::write(&md_path, md_report).expect("Failed to write AUDIT_GAP_REPORT.md");
            std::fs::write(&llms_path, llms_gaps).expect("Failed to write llms-gaps.txt");

            println!(
                "==> hh-docversion audit-claims: {} findings written to {}",
                all_findings.len(),
                md_path.display()
            );

            let has_blocking_gap = all_findings
                .iter()
                .any(|f| f.severity == "CRITICAL" || f.severity == "FAIL");

            if fail_on_gap && has_blocking_gap {
                eprintln!("==> GATE REJECTED: CRITICAL or FAIL claim gaps detected.");
                return ExitCode::from(1);
            }

            ExitCode::SUCCESS
        }
    }
}