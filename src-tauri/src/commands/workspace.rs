use crate::git_provider::SharedProviderRegistry;
use crate::state::{AppState, SourcePr, WorkspaceInfo, WorkspacePhase, WorkspaceStatus};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use super::helpers::{derive_branch_from_title, detect_default_branch, impl_branch_from, inject_shell_env, now_unix};

// ── Random workspace names ───────────────────────────────────────────

const ADJECTIVES: &[&str] = &[
    "swift", "calm", "bright", "gentle", "quiet", "bold", "keen", "warm",
    "cool", "wild", "deep", "soft", "sharp", "fresh", "still", "true",
    "pure", "rare", "wise", "fair", "clear", "proud", "quick", "neat",
    "slim", "vast", "vivid", "lucid", "amber", "misty",
];

const NOUNS: &[&str] = &[
    "oak", "elm", "pine", "fern", "moss", "reed", "sage", "mint",
    "jade", "onyx", "ruby", "opal", "hawk", "dove", "wolf", "bear",
    "fox", "lynx", "hare", "wren", "lark", "crow", "orca", "puma",
    "coral", "pearl", "ember", "dusk", "dawn", "vale",
];

fn random_workspace_name() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let h = hasher.finish();

    let adj = ADJECTIVES[(h as usize) % ADJECTIVES.len()];
    let noun = NOUNS[((h >> 16) as usize) % NOUNS.len()];
    format!("{}-{}", adj, noun)
}

// ── Workspace commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn create_workspace(
    repo_id: String,
    task_title: Option<String>,
    task_description: Option<String>,
    source_todo_id: Option<String>,
    custom_branch: Option<String>,
    phase: Option<WorkspacePhase>,
    state: State<'_, Arc<Mutex<AppState>>>,
    providers: State<'_, SharedProviderRegistry>,
) -> Result<WorkspaceInfo, String> {
    let (repo_path, gh_profile) = {
        let st = state.lock().map_err(|e| e.to_string())?;
        let repo = st.repos.get(&repo_id).ok_or("Repo not found")?;
        (repo.path.clone(), repo.gh_profile.clone())
    };

    let provider = providers.for_repo(&repo_path);

    // Resolve token early — if a profile is configured but the token can't be
    // obtained, fail immediately with diagnostic detail rather than silently
    // branching off stale data.
    let gh_token = match provider.resolve_token_strict(&gh_profile) {
        Ok(token) => token,
        Err(detail) => {
            return Err(format!(
                "Cannot authenticate as {} profile '{}'. \
                 Fix your auth or change the repo's profile.\n{}",
                provider.name(),
                gh_profile.as_deref().unwrap_or("unknown"),
                detail
            ));
        }
    };

    let base_branch = detect_default_branch(&repo_path)?;

    // Fetch origin so we branch from the latest remote state.
    // Provider handles URL rewriting for authentication.
    let mut fetch_cmd = provider.git_cmd_with_auth(&repo_path, &gh_token);
    fetch_cmd.args(["fetch", "origin", &base_branch]);
    let fetch_output = fetch_cmd
        .output()
        .map_err(|e| format!("Failed to run git fetch: {}", e))?;

    if !fetch_output.status.success() {
        let stderr = String::from_utf8_lossy(&fetch_output.stderr).to_string();
        let lower = stderr.to_lowercase();

        let hint = if lower.contains("repository not found") || lower.contains("could not read from remote") {
            if gh_profile.is_some() {
                "The configured GitHub profile may not have access to this repo. \
                 Try changing the profile in repo settings."
            } else {
                "No GitHub profile is set for this repo. \
                 Set one in repo settings so Korlap can authenticate."
            }
        } else if lower.contains("could not resolve host") {
            "Check your internet connection and try again."
        } else if lower.contains("permission denied") || lower.contains("authentication failed") {
            if gh_profile.is_some() {
                "Authentication failed. The token for this profile may be expired. \
                 Run `gh auth refresh` or change the profile in repo settings."
            } else {
                "Authentication failed. Set a GitHub profile in repo settings."
            }
        } else {
            "Check your git remote configuration and network connection."
        };

        return Err(format!(
            "Could not fetch from origin.\n{}\n\n{}",
            hint,
            stderr.trim()
        ));
    }

    let start_point = format!("origin/{}", base_branch);

    let worktree_base = {
        let st = state.lock().map_err(|e| e.to_string())?;
        st.worktree_dir()
    };

    // Resolve branch name. Priority:
    //   1. user-typed custom branch (errors if it already exists)
    //   2. derived from card title as <prefix>/<kebab-slug> with -2,-3,… on collision
    //   3. random `korlap/<adj>-<noun>` (also the fallback if (2) hits 10 collisions)
    //
    // `is_custom` flags branches that already have a meaningful name so the agent
    // skips its first-message rename prompt (see agent.rs `if !is_custom_branch`).
    let pick_random_branch = || -> Result<(String, String), String> {
        let mut name = random_workspace_name();
        for attempt in 0..10 {
            let branch = format!("korlap/{}", name);
            let check = std::process::Command::new("git")
                .args(["rev-parse", "--verify", &branch])
                .current_dir(&repo_path)
                .output()
                .map_err(|e| format!("Failed to run git: {}", e))?;
            let folder_exists = worktree_base.join(&name).exists();
            if !check.status.success() && !folder_exists {
                return Ok((name, branch));
            }
            if attempt == 9 {
                return Err("Could not generate a unique workspace name after 10 attempts".into());
            }
            name = format!(
                "{}-{}",
                random_workspace_name(),
                &Uuid::new_v4().to_string()[..4]
            );
        }
        unreachable!()
    };

    let (dir_name, branch, display_name, is_custom) = if let Some(ref cb) = custom_branch {
        let cb = cb.trim().to_string();
        if cb.is_empty() {
            return Err("Branch name cannot be empty".into());
        }

        let check = std::process::Command::new("git")
            .args(["rev-parse", "--verify", &cb])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;
        if check.status.success() {
            return Err(format!("Branch '{}' already exists", cb));
        }

        // Random dir name avoids filesystem issues with slashes in branch names.
        let dir = random_workspace_name();
        (dir, cb.clone(), cb, true)
    } else if let Some((prefix, slug)) = task_title.as_deref().and_then(derive_branch_from_title) {
        let mut chosen: Option<(String, String)> = None;
        for attempt in 0..10 {
            let suffixed_slug = if attempt == 0 {
                slug.clone()
            } else {
                format!("{}-{}", slug, attempt + 1)
            };
            let candidate_branch = format!("{}/{}", prefix, suffixed_slug);
            let check = std::process::Command::new("git")
                .args(["rev-parse", "--verify", &candidate_branch])
                .current_dir(&repo_path)
                .output()
                .map_err(|e| format!("Failed to run git: {}", e))?;
            let folder_exists = worktree_base.join(&suffixed_slug).exists();
            if !check.status.success() && !folder_exists {
                chosen = Some((suffixed_slug, candidate_branch));
                break;
            }
        }
        match chosen {
            Some((dir, br)) => (dir, br.clone(), br, true),
            None => {
                let (dir, br) = pick_random_branch()?;
                let display = dir.clone();
                (dir, br, display, false)
            }
        }
    } else {
        let (dir, br) = pick_random_branch()?;
        let display = dir.clone();
        (dir, br, display, false)
    };

    let id = Uuid::new_v4().to_string();

    // Worktree lives in app data dir, named after the workspace for human readability
    let worktree_path = worktree_base.join(&dir_name);

    std::fs::create_dir_all(worktree_path.parent().unwrap_or(&worktree_path))
        .map_err(|e| e.to_string())?;

    let output = std::process::Command::new("git")
        .args(["worktree", "add", "-b", &branch])
        .arg(&worktree_path)
        .arg(&start_point)
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("Failed to run git worktree add: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree add failed: {}", stderr.trim()));
    }

    let ws = WorkspaceInfo {
        id: id.clone(),
        name: display_name,
        branch,
        worktree_path: worktree_path.clone(),
        repo_id: repo_id.clone(),
        gh_profile,
        status: WorkspaceStatus::Waiting,
        created_at: now_unix(),
        task_title,
        task_description,
        source_todo_id,
        custom_branch: is_custom,
        provider_override: None,
        source_pr: None,
        source_prs: None,
        base_branch: None,
        phase: phase.unwrap_or_default(),
        archived: false,
    };

    // Check if there's a setup script to run
    let (setup_script, openspec_enabled) = {
        let st = state.lock().map_err(|e| e.to_string())?;
        let s = st.repo_settings.get(&repo_id);
        (
            s.map(|s| s.setup_script.clone()).unwrap_or_default(),
            s.map(|s| s.openspec_enabled).unwrap_or_default(),
        )
    };

    if !setup_script.trim().is_empty() {
        tracing::info!("Running setup script for workspace {}", ws.name);
        let mut setup_cmd = std::process::Command::new("zsh");
        setup_cmd.args(["-c", &setup_script]);
        setup_cmd.current_dir(&worktree_path);
        setup_cmd.env("KORLAP_WORKSPACE_NAME", &ws.name);
        setup_cmd.env("KORLAP_WORKSPACE_PATH", &worktree_path.to_string_lossy().to_string());
        setup_cmd.env("KORLAP_ROOT_PATH", repo_path.to_string_lossy().to_string());
        setup_cmd.env("KORLAP_DEFAULT_BRANCH", &base_branch);
        inject_shell_env(&mut setup_cmd);
        let output = setup_cmd
            .output()
            .map_err(|e| format!("Setup script failed to start: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Setup script failed: {}", stderr.trim());
            // Don't fail workspace creation — just log the warning
        }
    }

    // Initialize OpenSpec in the worktree if enabled and not already present.
    // Non-fatal: a failure here only logs a warning.
    if openspec_enabled && ws.phase == WorkspacePhase::Spec && !worktree_path.join("openspec").exists() {
        tracing::info!("Running openspec init for workspace {}", ws.name);
        let mut init_cmd = std::process::Command::new("openspec");
        init_cmd.arg("init").current_dir(&worktree_path);
        inject_shell_env(&mut init_cmd);
        match init_cmd.output() {
            Ok(out) if out.status.success() => {
                tracing::info!("openspec init succeeded for {}", ws.name);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!("openspec init failed: {}", stderr.trim());
            }
            Err(e) => {
                tracing::warn!("Could not spawn openspec (is it installed?): {}", e);
            }
        }
    }

    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.workspaces.insert(id, ws.clone());
    st.save_workspaces()?;

    tracing::info!("Created workspace {} ({})", ws.name, ws.id);
    Ok(ws)
}

#[tauri::command]
pub async fn create_workspace_from_pr(
    repo_id: String,
    pr_number: i64,
    state: State<'_, Arc<Mutex<AppState>>>,
    providers: State<'_, SharedProviderRegistry>,
) -> Result<WorkspaceInfo, String> {
    let (repo_path, gh_profile) = {
        let st = state.lock().map_err(|e| e.to_string())?;
        let repo = st.repos.get(&repo_id).ok_or("Repo not found")?;
        (repo.path.clone(), repo.gh_profile.clone())
    };

    let provider = providers.for_repo(&repo_path);
    let gh_token = provider.resolve_token(&gh_profile);

    // Fetch PR metadata via provider CLI
    let nwo = provider
        .extract_repo_id(&repo_path)
        .ok_or("Could not determine owner/repo from remote URL")?;

    let detail = provider.get_pr_detail(&repo_path, &nwo, pr_number, &gh_token)
        .map_err(|e| format!("Could not fetch PR #{}: {}", pr_number, e))?;

    let (pr_title, pr_branch, pr_base_branch, pr_url, pr_body) =
        (detail.title, detail.branch, detail.base_branch, detail.url, detail.body);

    // Fetch the PR ref (fork-safe: uses pull/<number>/head)
    let mut fetch_cmd = provider.git_cmd_with_auth(&repo_path, &gh_token);
    fetch_cmd.args(["fetch", "origin", &format!("pull/{}/head", pr_number)]);

    let fetch_output = fetch_cmd
        .output()
        .map_err(|e| format!("Failed to fetch PR #{}: {}", pr_number, e))?;

    if !fetch_output.status.success() {
        let stderr = String::from_utf8_lossy(&fetch_output.stderr);
        return Err(format!(
            "Could not fetch PR #{} from origin.\n{}",
            pr_number,
            stderr.trim()
        ));
    }

    // Generate workspace name and branch
    let worktree_base = {
        let st = state.lock().map_err(|e| e.to_string())?;
        st.worktree_dir()
    };

    let mut name = random_workspace_name();
    for attempt in 0..10 {
        let branch = format!("korlap/review-{}", name);
        let check = std::process::Command::new("git")
            .args(["rev-parse", "--verify", &branch])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        let folder_exists = worktree_base.join(&name).exists();

        if !check.status.success() && !folder_exists {
            break;
        }

        if attempt == 9 {
            return Err("Could not generate a unique workspace name after 10 attempts".into());
        }

        name = format!(
            "{}-{}",
            random_workspace_name(),
            &Uuid::new_v4().to_string()[..4]
        );
    }
    let branch = format!("korlap/review-{}", name);

    let id = Uuid::new_v4().to_string();
    let worktree_path = worktree_base.join(&name);

    std::fs::create_dir_all(worktree_path.parent().unwrap_or(&worktree_path))
        .map_err(|e| e.to_string())?;

    // Create worktree from FETCH_HEAD (the PR ref we just fetched)
    let output = std::process::Command::new("git")
        .args(["worktree", "add", "-b", &branch])
        .arg(&worktree_path)
        .arg("FETCH_HEAD")
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("Failed to run git worktree add: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree add failed: {}", stderr.trim()));
    }

    // Truncate PR body for task_description (keep it reasonable for agent context)
    let description = if pr_body.len() > 4000 {
        format!("{}…", &pr_body[..4000])
    } else {
        pr_body
    };

    let ws = WorkspaceInfo {
        id: id.clone(),
        name: name.clone(),
        branch,
        worktree_path: worktree_path.clone(),
        repo_id: repo_id.clone(),
        gh_profile,
        status: WorkspaceStatus::Waiting,
        created_at: now_unix(),
        task_title: Some(format!("Review PR #{}: {}", pr_number, pr_title)),
        task_description: if description.is_empty() { None } else { Some(description) },
        source_todo_id: None,
        custom_branch: true, // prevent agent from renaming
        provider_override: None,
        source_pr: Some(SourcePr {
            number: pr_number,
            branch: pr_branch,
            base_branch: pr_base_branch.clone(),
            url: pr_url,
            title: pr_title,
        }),
        source_prs: None,
        base_branch: Some(pr_base_branch),
        phase: WorkspacePhase::default(),
        archived: false,
    };

    // Run setup script if configured
    let (setup_script, default_branch) = {
        let st = state.lock().map_err(|e| e.to_string())?;
        let script = st.repo_settings
            .get(&repo_id)
            .map(|s| s.setup_script.clone())
            .unwrap_or_default();
        let db = detect_default_branch(&repo_path).unwrap_or_else(|_| "main".to_string());
        (script, db)
    };

    if !setup_script.trim().is_empty() {
        tracing::info!("Running setup script for PR review workspace {}", ws.name);
        let mut setup_cmd = std::process::Command::new("zsh");
        setup_cmd.args(["-c", &setup_script]);
        setup_cmd.current_dir(&worktree_path);
        setup_cmd.env("KORLAP_WORKSPACE_NAME", &ws.name);
        setup_cmd.env("KORLAP_WORKSPACE_PATH", &worktree_path.to_string_lossy().to_string());
        setup_cmd.env("KORLAP_ROOT_PATH", repo_path.to_string_lossy().to_string());
        setup_cmd.env("KORLAP_DEFAULT_BRANCH", &default_branch);
        inject_shell_env(&mut setup_cmd);
        let output = setup_cmd.output();
        if let Ok(ref out) = output {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!("Setup script failed: {}", stderr.trim());
            }
        }
    }

    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.workspaces.insert(id, ws.clone());
    st.save_workspaces()?;

    tracing::info!("Created PR review workspace {} for PR #{}", ws.name, pr_number);
    Ok(ws)
}

#[tauri::command]
pub async fn create_combo_workspace(
    repo_id: String,
    pr_numbers: Vec<i64>,
    state: State<'_, Arc<Mutex<AppState>>>,
    providers: State<'_, SharedProviderRegistry>,
) -> Result<WorkspaceInfo, String> {
    // Deduplicate and validate
    let mut seen = std::collections::HashSet::new();
    let pr_numbers: Vec<i64> = pr_numbers.into_iter().filter(|n| seen.insert(*n)).collect();
    if pr_numbers.len() < 2 {
        return Err(format!(
            "At least 2 PRs are required for a combo workspace (got {})",
            pr_numbers.len()
        ));
    }

    let (repo_path, gh_profile) = {
        let st = state.lock().map_err(|e| format!("Failed to lock app state: {}", e))?;
        let repo = st.repos.get(&repo_id).ok_or_else(|| format!(
            "Repo '{}' not found in app state — was it removed?", repo_id
        ))?;
        (repo.path.clone(), repo.gh_profile.clone())
    };

    let provider = providers.for_repo(&repo_path);
    let gh_token = provider.resolve_token(&gh_profile);

    let nwo = provider
        .extract_repo_id(&repo_path)
        .ok_or_else(|| format!(
            "Could not determine owner/repo from git remote in {}",
            repo_path.display()
        ))?;

    // Fetch PR details for all selected PRs
    let mut source_prs = Vec::with_capacity(pr_numbers.len());
    for &pr_num in &pr_numbers {
        let detail = provider
            .get_pr_detail(&repo_path, &nwo, pr_num, &gh_token)
            .map_err(|e| format!(
                "Could not fetch details for PR #{} from {}: {}", pr_num, nwo, e
            ))?;
        source_prs.push(SourcePr {
            number: pr_num,
            branch: detail.branch,
            base_branch: detail.base_branch,
            url: detail.url,
            title: detail.title,
        });
    }

    // Topologically sort PRs by dependency chain.
    // A PR depends on another if its base_branch matches the other's branch.
    // PRs targeting the default branch (or any branch not in the selection) come first.
    let base_branch = detect_default_branch(&repo_path)?;

    // Build a map from branch name to index for PRs in the selection
    let branch_to_idx: std::collections::HashMap<&str, usize> = source_prs
        .iter()
        .enumerate()
        .map(|(i, pr)| (pr.branch.as_str(), i))
        .collect();

    // Warn about missing dependencies: if a PR targets a branch that isn't the
    // default branch and isn't provided by another PR in the selection
    for pr in &source_prs {
        if pr.base_branch != base_branch && !branch_to_idx.contains_key(pr.base_branch.as_str()) {
            return Err(format!(
                "PR #{} ({}) targets branch '{}' which is not the default branch and not provided by any other selected PR. \
                 You may need to include its parent PR in the combo.",
                pr.number, pr.title, pr.base_branch
            ));
        }
    }

    // Topological sort via Kahn's algorithm
    let n = source_prs.len();
    let mut in_degree = vec![0usize; n];
    let mut dependents: Vec<Vec<usize>> = vec![vec![]; n];

    for (i, pr) in source_prs.iter().enumerate() {
        if let Some(&dep_idx) = branch_to_idx.get(pr.base_branch.as_str()) {
            // PR i depends on PR dep_idx (i's base is dep_idx's branch)
            in_degree[i] += 1;
            dependents[dep_idx].push(i);
        }
    }

    let mut queue: std::collections::VecDeque<usize> = in_degree
        .iter()
        .enumerate()
        .filter(|(_, &d)| d == 0)
        .map(|(i, _)| i)
        .collect();

    let mut sorted_indices = Vec::with_capacity(n);
    while let Some(idx) = queue.pop_front() {
        sorted_indices.push(idx);
        for &dep in &dependents[idx] {
            in_degree[dep] -= 1;
            if in_degree[dep] == 0 {
                queue.push_back(dep);
            }
        }
    }

    if sorted_indices.len() != n {
        let cycle_prs: Vec<String> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, &d)| d > 0)
            .map(|(i, _)| format!("#{} ({})", source_prs[i].number, source_prs[i].branch))
            .collect();
        return Err(format!(
            "Circular dependency detected among PRs: {}",
            cycle_prs.join(" → ")
        ));
    }

    // Reorder source_prs by topological order
    let source_prs: Vec<SourcePr> = sorted_indices
        .iter()
        .map(|&i| source_prs[i].clone())
        .collect();
    let mut fetch_cmd = provider.git_cmd_with_auth(&repo_path, &gh_token);
    fetch_cmd.args(["fetch", "origin", &base_branch]);
    let fetch_output = fetch_cmd
        .output()
        .map_err(|e| format!("Failed to spawn 'git fetch origin {}': {}", base_branch, e))?;
    if !fetch_output.status.success() {
        let stderr = String::from_utf8_lossy(&fetch_output.stderr);
        return Err(format!(
            "git fetch origin {} failed (exit {}):\n{}",
            base_branch,
            fetch_output.status.code().map_or("unknown".to_string(), |c| c.to_string()),
            stderr.trim()
        ));
    }

    // Generate unique workspace name
    let worktree_base = {
        let st = state.lock().map_err(|e| format!("Failed to lock app state: {}", e))?;
        st.worktree_dir()
    };

    let mut name = random_workspace_name();
    for attempt in 0..10 {
        let branch = format!("korlap/combo-{}", name);
        let check = std::process::Command::new("git")
            .args(["rev-parse", "--verify", &branch])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("Failed to spawn 'git rev-parse': {}", e))?;

        let folder_exists = worktree_base.join(&name).exists();

        if !check.status.success() && !folder_exists {
            break;
        }

        if attempt == 9 {
            return Err(format!(
                "Could not generate a unique combo workspace name after 10 attempts — \
                 last tried branch 'korlap/combo-{}' and path '{}'",
                name, worktree_base.join(&name).display()
            ));
        }

        name = format!(
            "{}-{}",
            random_workspace_name(),
            &Uuid::new_v4().to_string()[..4]
        );
    }
    let branch = format!("korlap/combo-{}", name);
    let id = Uuid::new_v4().to_string();
    let worktree_path = worktree_base.join(&name);
    let start_point = format!("origin/{}", base_branch);

    std::fs::create_dir_all(worktree_path.parent().unwrap_or(&worktree_path))
        .map_err(|e| format!(
            "Failed to create worktree parent directory '{}': {}",
            worktree_path.parent().map_or("?".to_string(), |p| p.display().to_string()),
            e
        ))?;

    let output = std::process::Command::new("git")
        .args(["worktree", "add", "-b", &branch])
        .arg(&worktree_path)
        .arg(&start_point)
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!(
            "Failed to spawn 'git worktree add -b {} {} {}': {}",
            branch, worktree_path.display(), start_point, e
        ))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git worktree add failed (exit {}):\n  branch: {}\n  path: {}\n  start: {}\n  error: {}",
            output.status.code().map_or("unknown".to_string(), |c| c.to_string()),
            branch,
            worktree_path.display(),
            start_point,
            stderr.trim()
        ));
    }

    // Sequentially fetch and merge each PR
    for (merge_idx, spr) in source_prs.iter().enumerate() {
        let pr_ref = format!("pull/{}/head", spr.number);
        let mut fetch_pr = provider.git_cmd_with_auth(&repo_path, &gh_token);
        fetch_pr.args(["fetch", "origin", &pr_ref]);
        let fetch_out = fetch_pr
            .output()
            .map_err(|e| format!(
                "Failed to spawn 'git fetch origin {}': {}", pr_ref, e
            ))?;

        if !fetch_out.status.success() {
            // Cleanup worktree + branch
            let _ = std::process::Command::new("git")
                .args(["worktree", "remove", "--force"])
                .arg(&worktree_path)
                .current_dir(&repo_path)
                .output();
            let _ = std::process::Command::new("git")
                .args(["branch", "-D", &branch])
                .current_dir(&repo_path)
                .output();
            let stderr = String::from_utf8_lossy(&fetch_out.stderr);
            let already = if merge_idx > 0 {
                let done: Vec<String> = source_prs[..merge_idx]
                    .iter()
                    .map(|p| format!("#{}", p.number))
                    .collect();
                format!("\n\nAlready merged: {}", done.join(", "))
            } else {
                String::new()
            };
            return Err(format!(
                "Could not fetch PR #{} ({}) via refs/pull/{}/head (exit {}):\n{}{}",
                spr.number, spr.title, spr.number,
                fetch_out.status.code().map_or("unknown".to_string(), |c| c.to_string()),
                stderr.trim(),
                already
            ));
        }

        // Resolve FETCH_HEAD to a commit SHA in repo_path (where fetch wrote it).
        // We can't use the symbolic "FETCH_HEAD" in the worktree because git writes
        // FETCH_HEAD to the main repo's .git dir, not the worktree's gitdir.
        let rev_parse = std::process::Command::new("git")
            .args(["rev-parse", "FETCH_HEAD"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!(
                "Failed to resolve FETCH_HEAD after fetching PR #{}: {}", spr.number, e
            ))?;
        if !rev_parse.status.success() {
            let stderr = String::from_utf8_lossy(&rev_parse.stderr);
            // Cleanup worktree + branch
            let _ = std::process::Command::new("git")
                .args(["worktree", "remove", "--force"])
                .arg(&worktree_path)
                .current_dir(&repo_path)
                .output();
            let _ = std::process::Command::new("git")
                .args(["branch", "-D", &branch])
                .current_dir(&repo_path)
                .output();
            return Err(format!(
                "Failed to resolve FETCH_HEAD for PR #{} ({}): {}",
                spr.number, spr.title, stderr.trim()
            ));
        }
        let commit_sha = String::from_utf8_lossy(&rev_parse.stdout).trim().to_string();

        let mut merge_cmd = std::process::Command::new("git");
        merge_cmd
            .args(["merge", &commit_sha, "--no-edit"])
            .current_dir(&worktree_path);
        inject_shell_env(&mut merge_cmd);
        let merge_out = merge_cmd
            .output()
            .map_err(|e| format!(
                "Failed to spawn 'git merge {}' for PR #{}: {}", commit_sha, spr.number, e
            ))?;

        if !merge_out.status.success() {
            // Extract conflicting file names via ls-files --unmerged (reliable during conflict)
            let unmerged_output = std::process::Command::new("git")
                .args(["ls-files", "--unmerged", "--deduplicate"])
                .current_dir(&worktree_path)
                .output();
            // ls-files --unmerged output: "<mode> <hash> <stage>\t<file>"
            let conflict_files: Vec<String> = unmerged_output
                .ok()
                .map(|o| {
                    let raw = String::from_utf8_lossy(&o.stdout);
                    let mut files: Vec<String> = raw
                        .lines()
                        .filter_map(|line| line.split('\t').nth(1).map(String::from))
                        .collect();
                    files.sort();
                    files.dedup();
                    files
                })
                .unwrap_or_default();

            // Also capture the merge output for additional context
            let merge_stdout = String::from_utf8_lossy(&merge_out.stdout);
            let merge_stderr = String::from_utf8_lossy(&merge_out.stderr);

            // Abort the failed merge
            let mut abort_cmd = std::process::Command::new("git");
            abort_cmd.args(["merge", "--abort"]).current_dir(&worktree_path);
            inject_shell_env(&mut abort_cmd);
            let _ = abort_cmd.output();

            // Cleanup worktree + branch
            let _ = std::process::Command::new("git")
                .args(["worktree", "remove", "--force"])
                .arg(&worktree_path)
                .current_dir(&repo_path)
                .output();
            let _ = std::process::Command::new("git")
                .args(["branch", "-D", &branch])
                .current_dir(&repo_path)
                .output();

            let file_section = if conflict_files.is_empty() {
                // Fallback: show raw merge output if ls-files didn't find anything
                let raw = format!("{}\n{}", merge_stdout.trim(), merge_stderr.trim());
                let raw = raw.trim();
                if raw.is_empty() {
                    String::new()
                } else {
                    format!("\n\ngit merge output:\n{}", raw)
                }
            } else {
                format!("\n\nConflicting files:\n{}", conflict_files
                    .iter()
                    .map(|f| format!("  • {}", f))
                    .collect::<Vec<_>>()
                    .join("\n"))
            };

            let already = if merge_idx > 0 {
                let done: Vec<String> = source_prs[..merge_idx]
                    .iter()
                    .map(|p| format!("#{}", p.number))
                    .collect();
                format!("\nSuccessfully merged before conflict: {}", done.join(", "))
            } else {
                String::new()
            };

            return Err(format!(
                "Merge conflict when adding PR #{} ({}) [{}/{}].{}{}",
                spr.number, spr.title,
                merge_idx + 1, source_prs.len(),
                file_section, already
            ));
        }
    }

    // Build task description from PR list
    let pr_summary: Vec<String> = source_prs
        .iter()
        .map(|p| format!("- PR #{}: {} ({})", p.number, p.title, p.url))
        .collect();
    let task_description = format!("Combined PRs for integration testing:\n{}", pr_summary.join("\n"));

    let pr_titles: Vec<String> = source_prs.iter().map(|p| format!("#{}", p.number)).collect();
    let task_title = format!("Combo: {}", pr_titles.join(" + "));

    let ws = WorkspaceInfo {
        id: id.clone(),
        name: name.clone(),
        branch,
        worktree_path: worktree_path.clone(),
        repo_id: repo_id.clone(),
        gh_profile,
        status: WorkspaceStatus::Waiting,
        created_at: now_unix(),
        task_title: Some(task_title),
        task_description: Some(task_description),
        source_todo_id: None,
        custom_branch: true,
        provider_override: None,
        source_pr: None,
        source_prs: Some(source_prs),
        base_branch: Some(base_branch.clone()),
        phase: WorkspacePhase::default(),
        archived: false,
    };

    // Run setup script if configured
    let setup_script = {
        let st = state.lock().map_err(|e| format!("Failed to lock app state: {}", e))?;
        st.repo_settings
            .get(&repo_id)
            .map(|s| s.setup_script.clone())
            .unwrap_or_default()
    };

    if !setup_script.trim().is_empty() {
        tracing::info!("Running setup script for combo workspace {}", ws.name);
        let mut setup_cmd = std::process::Command::new("zsh");
        setup_cmd.args(["-c", &setup_script]);
        setup_cmd.current_dir(&worktree_path);
        setup_cmd.env("KORLAP_WORKSPACE_NAME", &ws.name);
        setup_cmd.env("KORLAP_WORKSPACE_PATH", &worktree_path.to_string_lossy().to_string());
        setup_cmd.env("KORLAP_ROOT_PATH", repo_path.to_string_lossy().to_string());
        setup_cmd.env("KORLAP_DEFAULT_BRANCH", &base_branch);
        inject_shell_env(&mut setup_cmd);
        let output = setup_cmd.output();
        if let Ok(ref out) = output {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!("Setup script failed: {}", stderr.trim());
            }
        }
    }

    let mut st = state.lock().map_err(|e| format!("Failed to lock app state: {}", e))?;
    st.workspaces.insert(id, ws.clone());
    st.save_workspaces().map_err(|e| format!(
        "Combo workspace created successfully but failed to persist metadata: {}", e
    ))?;

    tracing::info!("Created combo workspace {} with {} PRs", ws.name, ws.source_prs.as_ref().map_or(0, |v| v.len()));
    Ok(ws)
}

/// Tear down the on-disk artifacts for a workspace: kill the agent process, kill
/// the workspace's terminals, drop it from LSP, run the optional `remove_script`,
/// and run `git worktree remove --force` (with a `prune` fallback). The workspace
/// entry in `state.workspaces` and its data files are left alone — callers
/// decide whether to also delete or merely archive.
fn cleanup_workspace_disk(
    workspace_id: &str,
    state: &Arc<Mutex<AppState>>,
    lsp_manager: &Arc<Mutex<crate::lsp::server::LspServerPool>>,
) -> Result<(), String> {
    let (worktree_path, repo_path, ws_name, repo_id) = {
        let mut st = state.lock().map_err(|e| e.to_string())?;

        if let Some(mut handle) = st.agents.remove(workspace_id) {
            let _ = handle.child.kill();
            let _ = handle.child.wait();
        }

        super::terminal::kill_workspace_terminals(&mut st.terminals, workspace_id);

        let ws = st
            .workspaces
            .get(workspace_id)
            .ok_or("Workspace not found")?;
        let repo = st.repos.get(&ws.repo_id).ok_or("Repo not found")?;
        (
            ws.worktree_path.clone(),
            repo.path.clone(),
            ws.name.clone(),
            ws.repo_id.clone(),
        )
    };

    if let Ok(mut mgr) = lsp_manager.lock() {
        mgr.remove_worktree(&repo_id, &worktree_path);
    }

    {
        let st = state.lock().map_err(|e| e.to_string())?;
        if let Some(settings) = st.repo_settings.get(&repo_id) {
            if !settings.remove_script.trim().is_empty() {
                tracing::info!("Running remove script for workspace {}", ws_name);
                let mut remove_cmd = std::process::Command::new("zsh");
                remove_cmd.args(["-c", &settings.remove_script]);
                remove_cmd.current_dir(&worktree_path);
                remove_cmd.env("KORLAP_WORKSPACE_NAME", &ws_name);
                remove_cmd.env(
                    "KORLAP_WORKSPACE_PATH",
                    &worktree_path.to_string_lossy().to_string(),
                );
                remove_cmd.env("KORLAP_ROOT_PATH", repo_path.to_string_lossy().to_string());
                inject_shell_env(&mut remove_cmd);
                let _ = remove_cmd.output();
            }
        }
    }

    if worktree_path.exists() {
        let output = std::process::Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&worktree_path)
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("Failed to remove worktree: {}", e))?;

        if !output.status.success() {
            let _ = std::process::Command::new("git")
                .args(["worktree", "prune"])
                .current_dir(&repo_path)
                .output();
        }
    } else {
        let _ = std::process::Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(&repo_path)
            .output();
    }

    Ok(())
}

#[tauri::command]
pub async fn remove_workspace(
    workspace_id: String,
    state: State<'_, Arc<Mutex<AppState>>>,
    lsp_manager: State<'_, Arc<Mutex<crate::lsp::server::LspServerPool>>>,
) -> Result<(), String> {
    cleanup_workspace_disk(&workspace_id, state.inner(), lsp_manager.inner())?;

    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.delete_workspace_data(&workspace_id);
    st.workspaces.remove(&workspace_id);
    st.save_workspaces()?;

    tracing::info!("Removed workspace {}", workspace_id);
    Ok(())
}

/// Like `remove_workspace`, but keep the entry in state and on disk (messages,
/// metadata). The card stays visible in the Done column as a historical record;
/// the worktree directory and agent process are gone.
#[tauri::command]
pub async fn archive_workspace(
    workspace_id: String,
    state: State<'_, Arc<Mutex<AppState>>>,
    lsp_manager: State<'_, Arc<Mutex<crate::lsp::server::LspServerPool>>>,
    app: AppHandle,
) -> Result<WorkspaceInfo, String> {
    cleanup_workspace_disk(&workspace_id, state.inner(), lsp_manager.inner())?;

    let snapshot = {
        let mut st = state.lock().map_err(|e| e.to_string())?;
        let ws = st
            .workspaces
            .get_mut(&workspace_id)
            .ok_or("Workspace not found")?;
        ws.archived = true;
        let snap = ws.clone();
        st.save_workspaces()?;
        snap
    };

    let _ = app.emit("workspace-updated", snapshot.clone());
    tracing::info!(
        "Archived workspace {} (worktree removed, entry retained)",
        workspace_id
    );
    Ok(snapshot)
}

#[tauri::command]
pub fn list_workspaces(
    repo_id: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<WorkspaceInfo>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state
        .workspaces
        .values()
        .filter(|w| w.repo_id == repo_id)
        .cloned()
        .collect())
}

// ── Branch commands ──────────────────────────────────────────────────

#[tauri::command]
pub fn set_workspace_phase(
    workspace_id: String,
    phase: WorkspacePhase,
    state: State<'_, Arc<Mutex<AppState>>>,
    app: AppHandle,
) -> Result<WorkspaceInfo, String> {
    let ws_clone = {
        let mut st = state.lock().map_err(|e| e.to_string())?;
        let ws = st
            .workspaces
            .get_mut(&workspace_id)
            .ok_or("Workspace not found")?;
        ws.phase = phase;
        let snapshot = ws.clone();
        st.save_workspaces()?;
        snapshot
    };

    let _ = app.emit("workspace-updated", ws_clone.clone());
    tracing::info!("Set workspace {} phase to {:?}", workspace_id, phase);
    Ok(ws_clone)
}

/// Advance a workspace from Spec → Implementing. When the repo has OpenSpec
/// enabled, this also branches off the current proposal branch into a fresh
/// `impl/<slug>` branch and switches the worktree onto it, so the proposal
/// and the implementation live on separate branches. With OpenSpec disabled
/// this is just a phase flip on the same branch.
#[tauri::command]
pub fn advance_to_implementation(
    workspace_id: String,
    state: State<'_, Arc<Mutex<AppState>>>,
    app: AppHandle,
) -> Result<WorkspaceInfo, String> {
    // 1) Snapshot what we need under the lock, then drop it before any git work.
    let (worktree_path, current_branch, current_phase, repo_id) = {
        let st = state.lock().map_err(|e| e.to_string())?;
        let ws = st
            .workspaces
            .get(&workspace_id)
            .ok_or("Workspace not found")?;
        (
            ws.worktree_path.clone(),
            ws.branch.clone(),
            ws.phase,
            ws.repo_id.clone(),
        )
    };

    let openspec_enabled = {
        let st = state.lock().map_err(|e| e.to_string())?;
        st.repo_settings
            .get(&repo_id)
            .map(|s| s.openspec_enabled)
            .unwrap_or_default()
    };

    // 2) If conditions are right, create the impl branch and switch the worktree.
    let new_branch: Option<String> = if openspec_enabled && current_phase == WorkspacePhase::Spec {
        let base = impl_branch_from(&current_branch);
        let mut chosen: Option<String> = None;
        for attempt in 0..10 {
            let candidate = if attempt == 0 {
                base.clone()
            } else {
                format!("{}-{}", base, attempt + 1)
            };

            let output = std::process::Command::new("git")
                .args(["checkout", "-b", &candidate])
                .current_dir(&worktree_path)
                .output()
                .map_err(|e| format!("Failed to run git checkout: {}", e))?;

            if output.status.success() {
                chosen = Some(candidate);
                break;
            }

            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // "already exists" is the only retry-able failure; everything else
            // (uncommitted-conflict, permissions, etc.) bubbles up.
            if !stderr.to_lowercase().contains("already exists") {
                return Err(format!("git checkout -b failed: {}", stderr.trim()));
            }
        }
        Some(chosen.ok_or_else(|| {
            format!(
                "Could not create a unique impl branch after 10 attempts (base: {})",
                base
            )
        })?)
    } else {
        None
    };

    // 3) Re-lock, mutate phase + branch, persist, emit.
    let ws_clone = {
        let mut st = state.lock().map_err(|e| e.to_string())?;
        let ws = st
            .workspaces
            .get_mut(&workspace_id)
            .ok_or("Workspace not found")?;
        ws.phase = WorkspacePhase::Implementing;
        if let Some(ref nb) = new_branch {
            ws.branch = nb.clone();
            ws.name = nb.clone();
        }
        let snapshot = ws.clone();
        st.save_workspaces()?;
        snapshot
    };

    let _ = app.emit("workspace-updated", ws_clone.clone());
    if let Some(ref nb) = new_branch {
        tracing::info!(
            "Advanced workspace {} to implementing on impl branch {}",
            workspace_id,
            nb
        );
    } else {
        tracing::info!(
            "Advanced workspace {} to implementing (openspec off, branch unchanged)",
            workspace_id
        );
    }
    Ok(ws_clone)
}

#[tauri::command]
pub fn rename_branch(
    workspace_id: String,
    new_name: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<WorkspaceInfo, String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    let ws = st
        .workspaces
        .get(&workspace_id)
        .ok_or("Workspace not found")?;


    let worktree_path = ws.worktree_path.clone();
    let fallback_branch = ws.branch.clone();

    crate::state::rename_git_branch(&worktree_path, &new_name, &fallback_branch)?;

    let ws = st
        .workspaces
        .get_mut(&workspace_id)
        .ok_or("Workspace not found")?;
    ws.branch = new_name.clone();
    ws.name = new_name;
    let ws_clone = ws.clone();
    st.save_workspaces()?;

    tracing::info!("Renamed workspace {} to {}", workspace_id, ws_clone.name);
    Ok(ws_clone)
}
