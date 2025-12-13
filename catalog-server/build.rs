use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Get version components
    let base_version = get_base_version().unwrap_or_else(|| "0.0".to_string());
    let commit_count = get_commit_count().unwrap_or(0);
    let git_hash = get_git_hash().unwrap_or_else(|| "unknown".to_string());
    let dirty_suffix = if is_repo_dirty() { "-dirty" } else { "" };

    // Full version: MAJOR.MINOR.COMMIT-COUNT
    let full_version = format!("{}.{}", base_version, commit_count);

    println!("cargo:rustc-env=APP_VERSION={}", full_version);
    println!("cargo:rustc-env=GIT_HASH={}{}", git_hash, dirty_suffix);

    // Rerun if VERSION file changes
    println!("cargo:rerun-if-changed=../VERSION");
}

fn get_base_version() -> Option<String> {
    // Try to read from VERSION file in repo root
    for version_path in &["../VERSION", "VERSION"] {
        if let Ok(content) = fs::read_to_string(version_path) {
            let version = content.trim().to_string();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }

    // Fall back to CARGO_PKG_VERSION (strips patch version if present)
    std::env::var("CARGO_PKG_VERSION").ok().map(|v| {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() >= 2 {
            format!("{}.{}", parts[0], parts[1])
        } else {
            v
        }
    })
}

fn get_commit_count() -> Option<u32> {
    // Check for COMMIT_COUNT env var first (for Docker builds)
    if let Ok(count) = std::env::var("COMMIT_COUNT") {
        if let Ok(n) = count.parse() {
            return Some(n);
        }
    }

    // Fall back to git command
    Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .and_then(|s| s.trim().parse().ok())
            } else {
                None
            }
        })
}

fn is_repo_dirty() -> bool {
    // Check GIT_DIRTY env var first (for Docker builds)
    if let Ok(val) = std::env::var("GIT_DIRTY") {
        return val == "1" || val.to_lowercase() == "true";
    }

    // Fall back to git command (for local builds)
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

fn get_git_hash() -> Option<String> {
    // Check for GIT_HASH env var first (e.g., from Docker build arg)
    if let Ok(hash) = std::env::var("GIT_HASH") {
        if !hash.is_empty() && hash != "unknown" {
            return Some(hash);
        }
    }

    // Try to read from .git directory (check parent for monorepo structure)
    for git_dir in &[".git", "../.git"] {
        if let Some(hash) = read_git_hash(Path::new(git_dir)) {
            return Some(hash);
        }
    }

    None
}

fn read_git_hash(git_dir: &Path) -> Option<String> {
    let head_path = git_dir.join("HEAD");
    let head_content = fs::read_to_string(&head_path).ok()?;
    let head_content = head_content.trim();

    if head_content.starts_with("ref: ") {
        // HEAD points to a ref, e.g., "ref: refs/heads/main"
        let ref_path = head_content.strip_prefix("ref: ")?;
        let full_ref_path = git_dir.join(ref_path);
        let hash = fs::read_to_string(&full_ref_path).ok()?;
        Some(hash.trim()[..7].to_string())
    } else {
        // Detached HEAD - contains the hash directly
        Some(head_content[..7].to_string())
    }
}
