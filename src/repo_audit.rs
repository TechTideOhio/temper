use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepoCheckResult {
    pub name: String,
    pub passed: bool,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepoAuditReport {
    pub repo_path: String,
    pub passed_checks: usize,
    pub total_checks: usize,
    pub passed: bool,
    pub checks: Vec<RepoCheckResult>,
}

pub fn audit_repository<P: AsRef<Path>>(repo_path: P) -> RepoAuditReport {
    let root = repo_path.as_ref();
    let mut checks = Vec::new();

    // 1. Required files
    let required = [
        "Cargo.toml",
        "README.md",
        "LICENSE",
        "SECURITY.md",
        "CONTRIBUTING.md",
        "CHANGELOG.md",
        "AGENTS.md",
    ];
    let mut missing = Vec::new();
    for f in required {
        if !root.join(f).is_file() {
            missing.push(f);
        }
    }
    checks.push(RepoCheckResult {
        name: "required_files".to_string(),
        passed: missing.is_empty(),
        severity: "blocker".to_string(),
        message: if missing.is_empty() {
            "all required repo metadata files present".to_string()
        } else {
            format!("missing required files: {}", missing.join(", "))
        },
    });

    // 2. Pinned toolchain
    let toolchain_file = root.join("rust-toolchain.toml");
    let toolchain_ok = if toolchain_file.is_file() {
        fs::read_to_string(&toolchain_file)
            .map(|s| s.contains("channel = \"stable\""))
            .unwrap_or(false)
    } else {
        false
    };
    checks.push(RepoCheckResult {
        name: "pinned_toolchain".to_string(),
        passed: toolchain_ok,
        severity: "blocker".to_string(),
        message: if toolchain_ok {
            "rust-toolchain.toml pins stable channel".to_string()
        } else {
            "rust-toolchain.toml missing or does not pin stable channel".to_string()
        },
    });

    // 3. Linguist vendored
    let gitattributes_file = root.join(".gitattributes");
    let linguist_ok = if gitattributes_file.is_file() {
        fs::read_to_string(&gitattributes_file)
            .map(|s| s.contains("linguist-vendored"))
            .unwrap_or(false)
    } else {
        false
    };
    checks.push(RepoCheckResult {
        name: "linguist_vendored".to_string(),
        passed: linguist_ok,
        severity: "graded".to_string(),
        message: if linguist_ok {
            ".gitattributes configures linguist-vendored rules".to_string()
        } else {
            ".gitattributes missing or lacks linguist-vendored".to_string()
        },
    });

    // 4. CI Pull Request Trigger
    let ci_file = root.join(".github/workflows/ci.yml");
    let ci_pr_ok = if ci_file.is_file() {
        fs::read_to_string(&ci_file)
            .map(|s| s.contains("pull_request:"))
            .unwrap_or(false)
    } else {
        false
    };
    checks.push(RepoCheckResult {
        name: "ci_pull_request_trigger".to_string(),
        passed: ci_pr_ok,
        severity: "blocker".to_string(),
        message: if ci_pr_ok {
            "ci.yml triggers on pull_request".to_string()
        } else {
            "ci.yml missing or does not trigger on pull_request".to_string()
        },
    });

    // 5. Pinned Action SHAs
    let workflows_dir = root.join(".github/workflows");
    let mut unpinned = Vec::new();
    if workflows_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&workflows_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map_or(false, |e| e == "yml" || e == "yaml")
                {
                    if let Ok(content) = fs::read_to_string(&p) {
                        for line in content.lines() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("uses:") {
                                if let Some((_, action)) = trimmed.split_once("uses:") {
                                    let action = action.trim();
                                    if !action.starts_with("./") && !action.starts_with("docker://")
                                    {
                                        if let Some((_, sha)) = action.split_once('@') {
                                            if sha.len() != 40
                                                || !sha.chars().all(|c| c.is_ascii_hexdigit())
                                            {
                                                unpinned.push(format!(
                                                    "{}: {}",
                                                    p.display(),
                                                    action
                                                ));
                                            }
                                        } else {
                                            unpinned.push(format!("{}: {}", p.display(), action));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    checks.push(RepoCheckResult {
        name: "pinned_action_shas".to_string(),
        passed: unpinned.is_empty(),
        severity: "blocker".to_string(),
        message: if unpinned.is_empty() {
            "all GitHub actions pinned to 40-character commit SHAs".to_string()
        } else {
            format!("unpinned actions detected: {}", unpinned.join("; "))
        },
    });

    // 6. Zero em-dash and en-dash typography check
    let mut typography_violations = Vec::new();
    scan_typography(root, &mut typography_violations);
    checks.push(RepoCheckResult {
        name: "typography_zero_em_dash".to_string(),
        passed: typography_violations.is_empty(),
        severity: "blocker".to_string(),
        message: if typography_violations.is_empty() {
            "zero em-dashes and en-dashes verified across codebase".to_string()
        } else {
            format!(
                "typography violations found in {} files: {}",
                typography_violations.len(),
                typography_violations.join(", ")
            )
        },
    });

    // 7. Secret hygiene
    let forbidden_secrets = [".env", ".env.local", "id_rsa", "id_ed25519"];
    let mut found_secrets = Vec::new();
    for sec in forbidden_secrets {
        if root.join(sec).exists() {
            found_secrets.push(sec);
        }
    }
    checks.push(RepoCheckResult {
        name: "secret_hygiene".to_string(),
        passed: found_secrets.is_empty(),
        severity: "blocker".to_string(),
        message: if found_secrets.is_empty() {
            "clean secret hygiene: no plaintext .env or ssh keys found".to_string()
        } else {
            format!("forbidden secret files found: {}", found_secrets.join(", "))
        },
    });

    // 8. Cargo release profile
    let cargo_file = root.join("Cargo.toml");
    let release_ok = if cargo_file.is_file() {
        fs::read_to_string(&cargo_file)
            .map(|s| s.contains("lto = true") && s.contains("strip = true"))
            .unwrap_or(false)
    } else {
        false
    };
    checks.push(RepoCheckResult {
        name: "cargo_release_profile".to_string(),
        passed: release_ok,
        severity: "graded".to_string(),
        message: if release_ok {
            "Cargo.toml configures optimized release profile (lto, strip)".to_string()
        } else {
            "Cargo.toml missing or lacks lto/strip in release profile".to_string()
        },
    });

    let passed_checks = checks.iter().filter(|c| c.passed).count();
    let total_checks = checks.len();
    let passed = checks.iter().all(|c| c.passed || c.severity == "graded");

    RepoAuditReport {
        repo_path: root.display().to_string(),
        passed_checks,
        total_checks,
        passed,
        checks,
    }
}

fn scan_typography(dir: &Path, violations: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str == "target" || name_str == ".git" {
                continue;
            }

            if path.is_dir() {
                scan_typography(&path, violations);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if matches!(ext, "rs" | "md" | "toml" | "json" | "jsonl") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if content.contains('\u{2014}') || content.contains('\u{2013}') {
                                violations.push(path.display().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
}
