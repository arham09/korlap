use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Git helpers ──────────────────────────────────────────────────────

pub fn detect_default_branch(repo_path: &Path) -> Result<String, String> {
    // Tier 1: origin HEAD symref (most reliable)
    let output = std::process::Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        let refname = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Some(branch) = refname.strip_prefix("refs/remotes/origin/") {
            return Ok(branch.to_string());
        }
    }

    // Tier 2: check which of origin/main, origin/master exists as REMOTE tracking refs.
    // Never fall back to local branches — workspaces must always branch from origin.
    for candidate in ["main", "master"] {
        let ref_name = format!("refs/remotes/origin/{}", candidate);
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--verify", &ref_name])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;
        if output.status.success() {
            return Ok(candidate.to_string());
        }
    }

    // No silent fallback — error out with actionable message.
    Err(
        "Could not detect default branch from remote. \
         No origin/HEAD, origin/main, or origin/master found. \
         Run `git remote set-head origin --auto` or check your remote configuration."
            .to_string(),
    )
}

pub fn repo_display_name(repo_path: &Path) -> String {
    repo_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| repo_path.display().to_string())
}

/// Build a Command that runs `script` inside the user's login shell.
/// Handles shell-specific arg differences:
///   zsh/bash: `<shell> -lic "<script>"`
///   fish:     `fish --login --interactive -c "<script>"`
fn login_shell_cmd(shell: &str, script: &str) -> std::process::Command {
    let shell_name = Path::new(shell)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut cmd = std::process::Command::new(shell);
    if shell_name == "fish" {
        cmd.args(["--login", "--interactive", "-c", script]);
    } else {
        // zsh, bash, and other POSIX-ish shells all accept -lic
        cmd.args(["-lic", script]);
    }
    cmd.stderr(std::process::Stdio::null());
    cmd
}

/// Extract the delimited value from noisy shell output.
/// Returns the trimmed text between the first pair of `delimiter` markers.
fn extract_delimited(stdout: &str, delimiter: &str) -> Option<String> {
    let mut parts = stdout.split(delimiter);
    let _before = parts.next(); // noise before first delimiter
    let value = parts.next()?;
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

/// Cached shell env values (resolved once on first call).
pub fn get_shell_env() -> &'static ShellEnv {
    use std::sync::OnceLock;
    static ENV: OnceLock<ShellEnv> = OnceLock::new();
    ENV.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        tracing::info!("Resolving shell environment using {}", shell);

        let ssh_auth_sock = std::env::var("SSH_AUTH_SOCK").ok().or_else(|| {
            std::process::Command::new("launchctl")
                .args(["getenv", "SSH_AUTH_SOCK"])
                .output()
                .ok()
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() { None } else { Some(s) }
                })
        });

        let home = std::env::var("HOME").ok();

        // Use interactive login shell so rc files are sourced — this is
        // where nvm/fnm/volta add their PATH entries.  Delimiters protect
        // against noisy shell output (motd, nvm "now using", etc.).
        let delimiter = "__KORLAP_ENV__";
        let path = login_shell_cmd(
                &shell,
                &format!("echo {delimiter}; echo $PATH; echo {delimiter}"),
            )
            .output()
            .ok()
            .and_then(|o| extract_delimited(&String::from_utf8_lossy(&o.stdout), delimiter));

        // Resolve absolute path to `claude` binary once, so we don't rely
        // on PATH lookup at every spawn (which can fail in sandboxed contexts).
        // `command -v` is POSIX and works in bash, zsh, and fish.
        let claude_path = login_shell_cmd(
                &shell,
                &format!("echo {delimiter}; command -v claude; echo {delimiter}"),
            )
            .output()
            .ok()
            .and_then(|o| extract_delimited(&String::from_utf8_lossy(&o.stdout), delimiter))
            .filter(|s| !s.contains("not found"));

        if claude_path.is_none() {
            tracing::warn!("Could not resolve `claude` binary path — agent spawn will likely fail");
        }

        // Resolve codex binary path (optional — only needed if user selects Codex provider)
        let codex_path = login_shell_cmd(
                &shell,
                &format!("echo {delimiter}; command -v codex; echo {delimiter}"),
            )
            .output()
            .ok()
            .and_then(|o| extract_delimited(&String::from_utf8_lossy(&o.stdout), delimiter))
            .filter(|s| !s.contains("not found"));

        if codex_path.is_some() {
            tracing::info!("Resolved codex binary: {:?}", codex_path);
        }

        // Capture full environment from interactive login shell so spawned
        // processes get all user env vars (CARGO_TARGET_DIR, GOPATH, etc.)
        // that a Tauri app launched from Finder/Dock would otherwise miss.
        let all_vars: HashMap<String, String> = login_shell_cmd(
                &shell,
                &format!("echo {delimiter}; /usr/bin/env; echo {delimiter}"),
            )
            .output()
            .ok()
            .and_then(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let mut parts = stdout.split(delimiter);
                let _before = parts.next();
                let env_section = parts.next()?;
                let mut vars = HashMap::new();
                let mut current_key = String::new();
                let mut current_val = String::new();
                for line in env_section.lines() {
                    if let Some(eq_pos) = line.find('=') {
                        let key = &line[..eq_pos];
                        // Valid env var names: alphanumeric + underscore, non-empty
                        if !key.is_empty()
                            && key
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        {
                            // Flush previous entry
                            if !current_key.is_empty() {
                                vars.insert(
                                    std::mem::take(&mut current_key),
                                    std::mem::take(&mut current_val),
                                );
                            }
                            current_key = key.to_string();
                            current_val = line[eq_pos + 1..].to_string();
                            continue;
                        }
                    }
                    // Continuation of a multi-line value
                    if !current_key.is_empty() {
                        current_val.push('\n');
                        current_val.push_str(line);
                    }
                }
                // Flush last entry
                if !current_key.is_empty() {
                    vars.insert(current_key, current_val);
                }
                Some(vars)
            })
            .unwrap_or_default();

        tracing::info!(
            "Captured {} env vars from login shell ({})",
            all_vars.len(),
            shell,
        );

        ShellEnv { ssh_auth_sock, home, path, claude_path, codex_path, all_vars }
    })
}

pub struct ShellEnv {
    pub ssh_auth_sock: Option<String>,
    pub home: Option<String>,
    pub path: Option<String>,
    pub claude_path: Option<String>,
    pub codex_path: Option<String>,
    /// Full environment captured from an interactive login shell.
    /// Contains all user env vars (CARGO_TARGET_DIR, GOPATH, etc.)
    /// that a Tauri app launched from Finder/Dock would otherwise miss.
    pub all_vars: HashMap<String, String>,
}

/// Strip ANSI escape sequences from a string.
pub fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we hit a letter (the terminator of an ANSI sequence)
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Inject the full user shell environment into a Command so that processes
/// spawned from a Finder/Dock-launched Tauri app behave like they were
/// started from a terminal (includes CARGO_TARGET_DIR, GOPATH, etc.).
pub fn inject_shell_env(cmd: &mut std::process::Command) {
    let env = get_shell_env();

    // Apply all env vars captured from the interactive login shell.
    cmd.envs(&env.all_vars);

    // Fallback: SSH_AUTH_SOCK from launchctl if not present in shell env
    // (some setups only expose it via launchd, not the shell profile).
    if !env.all_vars.contains_key("SSH_AUTH_SOCK") {
        if let Some(ref sock) = env.ssh_auth_sock {
            cmd.env("SSH_AUTH_SOCK", sock);
        }
    }
}

// ── Title → branch helpers ───────────────────────────────────────────

/// Convert a title into a kebab-case slug. Lowercases ASCII alphanumerics,
/// collapses runs of non-alphanumerics into a single `-`, trims trailing `-`.
/// Mirrors the TS `slugifyTitle` in +page.svelte.
pub fn slugify_title(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_hyphen = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_hyphen = false;
        } else if !prev_hyphen {
            out.push('-');
            prev_hyphen = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

const BRANCH_PREFIXES: &[&str] = &["chore", "feat", "fix"];
const PREFIX_SEPARATORS: &[char] = &[':', '-', '/'];
const SLUG_MAX_LEN: usize = 60;

/// Pick a conventional-commit prefix from the start of a card title.
/// Returns (prefix, rest) where `prefix` ∈ {"feat","fix","chore"}; defaults to
/// "feat" when no prefix word is found at the start. The prefix word must be
/// followed by `:`, `-`, `/`, ASCII whitespace, or end-of-input — so titles
/// like "fixing the bug" do NOT match `fix`.
pub fn extract_prefix(title: &str) -> (&'static str, &str) {
    let trimmed = title.trim_start();

    for &candidate in BRANCH_PREFIXES {
        let n = candidate.len();
        let head = match trimmed.get(..n) {
            Some(h) => h,
            None => continue,
        };
        if !head.eq_ignore_ascii_case(candidate) {
            continue;
        }
        let after = &trimmed[n..];
        match after.chars().next() {
            None => return (candidate, ""),
            Some(c) if c.is_ascii_whitespace() || PREFIX_SEPARATORS.contains(&c) => {
                let rest = after.trim_start_matches(|c: char| {
                    c.is_ascii_whitespace() || PREFIX_SEPARATORS.contains(&c)
                });
                return (candidate, rest);
            }
            Some(_) => continue,
        }
    }

    ("feat", trimmed)
}

/// Compose `extract_prefix` + `slugify_title` + length cap. Returns `None`
/// when the title can't yield a meaningful slug (empty, all symbols, or just
/// a bare prefix like "fix:") so callers can fall back to random naming.
pub fn derive_branch_from_title(title: &str) -> Option<(&'static str, String)> {
    let (prefix, rest) = extract_prefix(title);
    let mut slug = slugify_title(rest);
    if slug.is_empty() {
        return None;
    }
    // Slug is ASCII after slugify_title, so byte-len == char-len; truncate is safe.
    if slug.len() > SLUG_MAX_LEN {
        slug.truncate(SLUG_MAX_LEN);
        while slug.ends_with('-') {
            slug.pop();
        }
        if slug.is_empty() {
            return None;
        }
    }
    Some((prefix, slug))
}

/// Compute the implementation-branch name from a proposal-branch name. Strips
/// the segment before the first `/` (the conventional prefix `feat/`/`fix/`/
/// `chore/`/`korlap/`) and replaces it with `impl/`. Branches without a `/`
/// just get `impl/` prepended.
pub fn impl_branch_from(original: &str) -> String {
    let slug = original
        .split_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or(original);
    format!("impl/{}", slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_prefix_explicit_colon() {
        assert_eq!(extract_prefix("feat: add login button"), ("feat", "add login button"));
    }

    #[test]
    fn extract_prefix_space_separator() {
        assert_eq!(extract_prefix("Fix memory leak"), ("fix", "memory leak"));
    }

    #[test]
    fn extract_prefix_dash_separator() {
        assert_eq!(extract_prefix("Chore - update deps"), ("chore", "update deps"));
    }

    #[test]
    fn extract_prefix_slash_separator() {
        assert_eq!(extract_prefix("feat/foo-bar"), ("feat", "foo-bar"));
    }

    #[test]
    fn extract_prefix_default_feat() {
        assert_eq!(extract_prefix("Update README"), ("feat", "Update README"));
    }

    #[test]
    fn extract_prefix_substring_no_match() {
        assert_eq!(extract_prefix("fixing the bug"), ("feat", "fixing the bug"));
        assert_eq!(extract_prefix("feature add button"), ("feat", "feature add button"));
        assert_eq!(extract_prefix("chores list"), ("feat", "chores list"));
    }

    #[test]
    fn extract_prefix_uppercase() {
        assert_eq!(extract_prefix("FEAT: x"), ("feat", "x"));
        assert_eq!(extract_prefix("CHORE/y"), ("chore", "y"));
    }

    #[test]
    fn extract_prefix_leading_whitespace() {
        assert_eq!(extract_prefix("  fix: x"), ("fix", "x"));
    }

    #[test]
    fn extract_prefix_only_prefix() {
        assert_eq!(extract_prefix("fix"), ("fix", ""));
        assert_eq!(extract_prefix("feat"), ("feat", ""));
        assert_eq!(extract_prefix("chore"), ("chore", ""));
    }

    #[test]
    fn extract_prefix_utf8_safe() {
        // No prefix match; multi-byte chars must not panic on byte-slice indexing.
        let (p, r) = extract_prefix("修复 bug");
        assert_eq!(p, "feat");
        assert_eq!(r, "修复 bug");
    }

    #[test]
    fn slugify_title_basic() {
        assert_eq!(slugify_title("Hello World!"), "hello-world");
        assert_eq!(slugify_title("foo  bar"), "foo-bar");
        assert_eq!(slugify_title("---"), "");
        assert_eq!(slugify_title(""), "");
    }

    #[test]
    fn derive_branch_from_title_basic() {
        assert_eq!(
            derive_branch_from_title("feat: add login button"),
            Some(("feat", "add-login-button".to_string()))
        );
    }

    #[test]
    fn derive_branch_from_title_default_prefix() {
        assert_eq!(
            derive_branch_from_title("Update README"),
            Some(("feat", "update-readme".to_string()))
        );
    }

    #[test]
    fn derive_branch_from_title_fix_dash() {
        assert_eq!(
            derive_branch_from_title("Chore - update deps"),
            Some(("chore", "update-deps".to_string()))
        );
    }

    #[test]
    fn derive_branch_from_title_empty_slug() {
        assert_eq!(derive_branch_from_title("!!!"), None);
        assert_eq!(derive_branch_from_title(""), None);
        assert_eq!(derive_branch_from_title("fix:"), None);
    }

    #[test]
    fn derive_branch_from_title_truncates() {
        let title = "a".repeat(200);
        let (prefix, slug) = derive_branch_from_title(&title).unwrap();
        assert_eq!(prefix, "feat");
        assert!(slug.len() <= SLUG_MAX_LEN);
        assert!(!slug.ends_with('-'));
    }

    #[test]
    fn impl_branch_from_strips_feat_prefix() {
        assert_eq!(impl_branch_from("feat/add-login-button"), "impl/add-login-button");
    }

    #[test]
    fn impl_branch_from_strips_korlap_prefix() {
        assert_eq!(impl_branch_from("korlap/clever-fox"), "impl/clever-fox");
    }

    #[test]
    fn impl_branch_from_no_slash() {
        assert_eq!(impl_branch_from("myfeature"), "impl/myfeature");
    }

    #[test]
    fn impl_branch_from_keeps_extra_slashes() {
        // Multi-slash branches: only the leading segment is replaced.
        assert_eq!(impl_branch_from("feat/sub/foo"), "impl/sub/foo");
    }

    #[test]
    fn impl_branch_from_already_impl() {
        // No-op for already-impl branches; collision retry handles the case.
        assert_eq!(impl_branch_from("impl/foo"), "impl/foo");
    }
}
