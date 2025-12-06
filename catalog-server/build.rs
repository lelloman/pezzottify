use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let git_hash = get_git_hash().unwrap_or_else(|| "unknown".to_string());
    let dirty_suffix = if is_repo_dirty() { "-dirty" } else { "" };
    println!("cargo:rustc-env=GIT_HASH={}{}", git_hash, dirty_suffix);
    // No rerun-if-changed: let cargo rerun build.rs on any source change
    // This ensures dirty detection works for unstaged changes
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
