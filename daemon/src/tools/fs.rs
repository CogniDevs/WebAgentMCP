use colored::Colorize;
use ignore::gitignore::GitignoreBuilder;
use regex::{Captures, Regex};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct SecretPattern {
    regex: Regex,
    replacement: &'static str,
}

static PATTERNS: OnceLock<Vec<SecretPattern>> = OnceLock::new();
static HIGH_ENTROPY_TOKEN_RE: OnceLock<Regex> = OnceLock::new();

fn get_secret_patterns() -> &'static Vec<SecretPattern> {
    PATTERNS.get_or_init(|| {
        vec![
            SecretPattern {
                regex: Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap(),
                replacement: "[REDACTED_OPENAI_KEY]",
            },
            SecretPattern {
                regex: Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,}").unwrap(),
                replacement: "[REDACTED_ANTHROPIC_KEY]",
            },
            SecretPattern {
                regex: Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),
                replacement: "[REDACTED_GITHUB_TOKEN]",
            },
            SecretPattern {
                regex: Regex::new(r"glpat-[a-zA-Z0-9_-]{20}").unwrap(),
                replacement: "[REDACTED_GITLAB_TOKEN]",
            },
            SecretPattern {
                regex: Regex::new(r"hf_[a-zA-Z0-9]{34,}").unwrap(),
                replacement: "[REDACTED_HUGGINGFACE_TOKEN]",
            },
            SecretPattern {
                regex: Regex::new(r"xox[baprs]-[a-zA-Z0-9]{10,}").unwrap(),
                replacement: "[REDACTED_SLACK_TOKEN]",
            },
            SecretPattern {
                regex: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
                replacement: "[REDACTED_AWS_KEY]",
            },
            SecretPattern {
                regex: Regex::new(r"AIzaSy[a-zA-Z0-9_-]{33}").unwrap(),
                replacement: "[REDACTED_GOOGLE_KEY]",
            },
            SecretPattern {
                regex: Regex::new(r"sk_live_[a-zA-Z0-9]{24}").unwrap(),
                replacement: "[REDACTED_STRIPE_KEY]",
            },
            SecretPattern {
                regex: Regex::new(r"eyJ[a-zA-Z0-9_-]{10,}\.eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}").unwrap(),
                replacement: "[REDACTED_JWT_TOKEN]",
            },
            SecretPattern {
                regex: Regex::new(r"(?i)(postgres|postgresql|mongodb|mongodb\+srv|mysql|redis)://[a-zA-Z0-9_]+:[^@\s]+@[a-zA-Z0-9_.-]+:[0-9]+/[a-zA-Z0-9_.-]+").unwrap(),
                replacement: "[REDACTED_CONNECTION_STRING]",
            },
            SecretPattern {
                regex: Regex::new(r"(?s)-----BEGIN [A-Z ]+ PRIVATE KEY-----.*?-----END [A-Z ]+ PRIVATE KEY-----").unwrap(),
                replacement: "[REDACTED_PRIVATE_KEY]",
            },
            SecretPattern {
                regex: Regex::new(r#"(?im)^(\s*(?:[\w_.]*?(?:api[_-]?key|secret|password|token|bearer)[\w_.]*?)\s*[:=]\s*)"[^"]{8,}""#).unwrap(),
                replacement: r#"$1"[REDACTED_SECRET]""#,
            },
            SecretPattern {
                regex: Regex::new(r#"(?im)^(\s*(?:[\w_.]*?(?:api[_-]?key|secret|password|token|bearer)[\w_.]*?)\s*)'[^']{8,}'"#).unwrap(),
                replacement: r#"$1'[REDACTED_SECRET]'"#,
            },
        ]
    })
}

fn calculate_entropy(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let bounded = if text.len() > 256 { &text[..256] } else { text };
    let mut frequency = HashMap::new();
    let total = bounded.chars().count() as f64;
    for c in bounded.chars() {
        *frequency.entry(c).or_insert(0usize) += 1;
    }
    let mut entropy = 0.0;
    for &count in frequency.values() {
        let p = count as f64 / total;
        entropy -= p * p.log2();
    }
    entropy
}

pub fn sanitize_secrets(text: &str) -> String {
    let mut result = text.to_string();
    for pattern in get_secret_patterns() {
        result = pattern.regex.replace_all(&result, pattern.replacement).to_string();
    }

    let token_re = HIGH_ENTROPY_TOKEN_RE.get_or_init(|| {
        Regex::new(r#"\b[a-zA-Z0-9_\-\+/=]{21,256}\b"#).unwrap()
    });

    result = token_re
        .replace_all(&result, |caps: &Captures| {
            let matched = caps.get(0).map_or("", |m| m.as_str());
            if matched.starts_with("[REDACTED_") {
                matched.to_string()
            } else if calculate_entropy(matched) > 4.5 {
                "[REDACTED_HIGH_ENTROPY_SECRET]".to_string()
            } else {
                matched.to_string()
            }
        })
        .to_string();

    result
}

pub fn secure_path(base_dir: &str, target_path: &str) -> Result<PathBuf, String> {
    let clean_base_str = base_dir.strip_prefix(r"\\?\").unwrap_or(base_dir);
    let clean = target_path.trim().trim_start_matches(|c| c == '/' || c == '\\');
    let full = Path::new(clean_base_str).join(clean);
    let canonical_base = Path::new(clean_base_str)
        .canonicalize()
        .map_err(|e| format!("Base path error: {}", e))?;

    if !full.exists() {
        if let Some(parent) = full.parent() {
            if let Ok(canon_parent) = parent.canonicalize() {
                if !canon_parent.starts_with(&canonical_base) {
                    return Err("Path traversal detected".to_string());
                }
            }
        }
        return Ok(full);
    }

    let canonical_target = full
        .canonicalize()
        .map_err(|e| format!("Target path error: {}", e))?;

    if !canonical_target.starts_with(&canonical_base) {
        return Err("Path traversal detected".to_string());
    }

    Ok(canonical_target)
}

pub fn print_terminal_diff(path: &str, old_content: &str, new_content: &str) {
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut additions = 0usize;
    let mut deletions = 0usize;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => deletions += 1,
            ChangeTag::Equal => {}
        }
    }

    let is_new = old_content.is_empty() && !new_content.is_empty();
    let status_tag = if is_new { " (new file)" } else { "" };

    println!();
    println!(
        "{} {}{}",
        "DIFF:".bright_cyan().bold(),
        path.bright_white().bold(),
        status_tag.dimmed()
    );
    println!(
        "  {}  {}",
        format!("+{}", additions).green(),
        format!("-{}", deletions).red()
    );
    println!("{}", "──────────────────────────────────────────────────".bright_black());

    let mut printed = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                print!("{} {}", "-".red().bold(), change.value().red());
                printed += 1;
            }
            ChangeTag::Insert => {
                print!("{} {}", "+".green().bold(), change.value().green());
                printed += 1;
            }
            ChangeTag::Equal => {
                if printed > 0 && printed < 8 {
                    print!("  {}", change.value().dimmed());
                }
            }
        }
    }

    if !new_content.ends_with('\n') {
        println!();
    }
    println!("{}", "──────────────────────────────────────────────────".bright_black());
}

struct TreeNode {
    name: String,
    is_dir: bool,
    children: Vec<TreeNode>,
}

fn build_tree(dir: &Path, gitignore: Option<&ignore::gitignore::Gitignore>) -> Option<TreeNode> {
    if !dir.exists() || !dir.is_dir() {
        return None;
    }

    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.to_string_lossy().to_string());

    let mut node = TreeNode {
        name,
        is_dir: true,
        children: Vec::new(),
    };

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Some(node),
    };

    let mut items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    items.sort_by(|a, b| {
        let a_dir = a.file_type().map_or(false, |ft| ft.is_dir());
        let b_dir = b.file_type().map_or(false, |ft| ft.is_dir());
        if a_dir != b_dir {
            b_dir.cmp(&a_dir)
        } else {
            a.file_name().cmp(&b.file_name())
        }
    });

    let default_ignore = [
        ".git", "node_modules", "target", "dist", "build", "out",
        "__pycache__", ".venv", "venv", ".idea", ".vscode",
    ];

    for entry in items {
        let path = entry.path();
        let is_dir = entry.file_type().map_or(false, |ft| ft.is_dir());
        let file_name = entry.file_name().to_string_lossy().to_string();

        if default_ignore.contains(&file_name.as_str()) {
            continue;
        }

        if let Some(gi) = gitignore {
            if gi.matched_path_or_any_parents(&path, is_dir).is_ignore() {
                continue;
            }
        }

        if is_dir {
            if let Some(child_node) = build_tree(&path, gitignore) {
                node.children.push(child_node);
            }
        } else {
            node.children.push(TreeNode {
                name: file_name,
                is_dir: false,
                children: Vec::new(),
            });
        }
    }

    Some(node)
}

fn render_ascii_tree(node: &TreeNode, indent: &str, is_last: bool, is_root: bool) -> Vec<String> {
    let mut lines = Vec::new();

    if is_root {
        lines.push(format!("{}/", node.name));
    } else {
        let prefix = if is_last { "└── " } else { "├── " };
        let display = if node.is_dir {
            format!("{}/", node.name)
        } else {
            node.name.clone()
        };
        lines.push(format!("{}{}{}", indent, prefix, display));
    }

    let next_indent = if is_root {
        ""
    } else if is_last {
        "    "
    } else {
        "│   "
    };

    let new_indent = format!("{}{}", indent, next_indent);
    let child_count = node.children.len();

    for (idx, child) in node.children.iter().enumerate() {
        let last_child = idx == child_count - 1;
        lines.extend(render_ascii_tree(child, &new_indent, last_child, false));
    }

    lines
}

pub fn view_dir(workspace: &str, sub_dir: &str) -> Result<String, String> {
    let clean_workspace = workspace.strip_prefix(r"\\?\").unwrap_or(workspace);
    let clean_sub = sub_dir.trim().trim_matches(|c| c == '.' || c == '/' || c == '\\');

    let target = if clean_sub.is_empty() {
        PathBuf::from(clean_workspace)
    } else {
        secure_path(clean_workspace, sub_dir)?
    };

    if !target.exists() || !target.is_dir() {
        return Err("Directory does not exist".to_string());
    }

    let mut gi_builder = GitignoreBuilder::new(Path::new(clean_workspace));
    let gitignore_file = Path::new(clean_workspace).join(".gitignore");
    if gitignore_file.exists() {
        let _ = gi_builder.add(&gitignore_file);
    }
    let gitignore = gi_builder.build().ok();

    match build_tree(&target, gitignore.as_ref()) {
        Some(tree_root) => {
            let lines = render_ascii_tree(&tree_root, "", false, true);
            if lines.is_empty() {
                Ok("Directory is empty".to_string())
            } else {
                let count = lines.len();
                if count > 400 {
                    let mut truncated = lines[..400].to_vec();
                    truncated.push(format!("... [Showing first 400 of {} items listed]", count));
                    Ok(truncated.join("\n"))
                } else {
                    Ok(lines.join("\n"))
                }
            }
        }
        None => Err("Failed to scan directory".to_string()),
    }
}

pub fn read_file(workspace: &str, path: &str) -> Result<String, String> {
    let clean_workspace = workspace.strip_prefix(r"\\?\").unwrap_or(workspace);
    let full = secure_path(clean_workspace, path)?;
    if !full.exists() {
        return Err("File not found".to_string());
    }
    let raw = fs::read_to_string(&full).map_err(|e| e.to_string())?;
    let content = sanitize_secrets(&raw);

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if total_lines > 250 {
        let slice = lines[..250].join("\n");
        return Ok(format!(
            "{}\n\n[Showing lines 1-250 of {}. File is preserved intact. To inspect subsequent lines, invoke: <tool='read_lines' path='{}' start='251' end='500'></tool>]",
            slice,
            total_lines,
            path
        ));
    }

    Ok(content)
}

pub fn read_lines(workspace: &str, path: &str, start: usize, end: usize) -> Result<String, String> {
    let clean_workspace = workspace.strip_prefix(r"\\?\").unwrap_or(workspace);
    let full = secure_path(clean_workspace, path)?;
    if !full.exists() {
        return Err("File not found".to_string());
    }
    let content = fs::read_to_string(&full).map_err(|e| e.to_string())?;
    let sanitized = sanitize_secrets(&content);
    let lines: Vec<&str> = sanitized.lines().collect();

    if lines.is_empty() {
        return Ok("File is empty".to_string());
    }

    let total = lines.len();
    let actual_start = start.max(1);
    let actual_end = end.min(total).max(actual_start);

    let mut output = Vec::new();
    output.push(format!("--- Lines {} to {} of {} ---", actual_start, actual_end, total));

    for idx in (actual_start - 1)..actual_end {
        if let Some(line) = lines.get(idx) {
            output.push(format!("{}: {}", idx + 1, line));
        }
    }

    if actual_end < total {
        let next_start = actual_end + 1;
        let next_end = (next_start + 150).min(total);
        output.push(format!(
            "\n[To view next lines, invoke: <tool='read_lines' path='{}' start='{}' end='{}'></tool>]",
            path, next_start, next_end
        ));
    }

    Ok(output.join("\n"))
}

pub fn strip_code_fences(content: &str) -> String {
    let trimmed = content.trim_start_matches('\n');
    let re_fence = Regex::new(r#"(?s)^```[a-zA-Z0-9_\-\.]*\r?\n(.*?)\r?\n```\s*$"#).unwrap();
    if let Some(caps) = re_fence.captures(trimmed) {
        if let Some(inner) = caps.get(1) {
            return inner.as_str().to_string();
        }
    }
    trimmed.to_string()
}

pub fn write_file(workspace: &str, path: &str, content: &str) -> Result<String, String> {
    let clean_workspace = workspace.strip_prefix(r"\\?\").unwrap_or(workspace);
    let full = secure_path(clean_workspace, path)?;
    let old_content = if full.exists() {
        fs::read_to_string(&full).unwrap_or_default()
    } else {
        String::new()
    };

    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let cleaned = strip_code_fences(content);
    print_terminal_diff(path, &old_content, &cleaned);

    fs::write(&full, cleaned).map_err(|e| e.to_string())?;
    Ok(format!("Successfully wrote {}", path))
}

pub fn patch_file(workspace: &str, path: &str, search: &str, replace: &str) -> Result<String, String> {
    let clean_workspace = workspace.strip_prefix(r"\\?\").unwrap_or(workspace);
    let full = secure_path(clean_workspace, path)?;
    if !full.exists() {
        return Err("File not found".to_string());
    }
    let content = fs::read_to_string(&full).map_err(|e| e.to_string())?;

    let norm_content = content.replace("\r\n", "\n");
    let norm_search = search.replace("\r\n", "\n");
    let norm_replace = replace.replace("\r\n", "\n");

    if !norm_content.contains(&norm_search) {
        return Err("Search block not found exactly as written".to_string());
    }
    let matches_count = norm_content.matches(&norm_search).count();
    if matches_count > 1 {
        return Err("Search block matches multiple locations, specify more surrounding context".to_string());
    }

    let updated = norm_content.replacen(&norm_search, &norm_replace, 1);
    let final_content = if content.contains("\r\n") {
        updated.replace('\n', "\r\n")
    } else {
        updated
    };

    print_terminal_diff(path, &content, &final_content);
    fs::write(&full, final_content).map_err(|e| e.to_string())?;
    Ok(format!("Patch applied successfully to {}", path))
}

pub fn replace_bounded(
    workspace: &str,
    path: &str,
    start_line: usize,
    end_line: usize,
    search: &str,
    replace: &str,
) -> Result<String, String> {
    let clean_workspace = workspace.strip_prefix(r"\\?\").unwrap_or(workspace);
    let full = secure_path(clean_workspace, path)?;
    if !full.exists() {
        return Err("File not found".to_string());
    }
    let content = fs::read_to_string(&full).map_err(|e| e.to_string())?;

    if start_line == 0 && end_line == 0 {
        return patch_file(clean_workspace, path, search, replace);
    }

    let norm_content = content.replace("\r\n", "\n");
    let norm_search = search.replace("\r\n", "\n");
    let norm_replace = replace.replace("\r\n", "\n");

    let lines: Vec<&str> = norm_content.lines().collect();
    if lines.is_empty() {
        return Err("File is empty".to_string());
    }

    let s = start_line.saturating_sub(1).min(lines.len());
    let e = end_line.min(lines.len()).max(s);

    let prefix = if s > 0 {
        lines[..s].join("\n") + "\n"
    } else {
        String::new()
    };

    let slice = lines[s..e].join("\n");

    let suffix = if e < lines.len() {
        "\n".to_string() + &lines[e..].join("\n")
    } else {
        String::new()
    };

    if !slice.contains(&norm_search) {
        return Err(format!("Search block not found within specified lines {}-{}", start_line, end_line));
    }

    let patched_slice = slice.replacen(&norm_search, &norm_replace, 1);
    let updated = format!("{}{}{}", prefix, patched_slice, suffix);

    let final_content = if content.contains("\r\n") {
        updated.replace('\n', "\r\n")
    } else {
        updated
    };

    print_terminal_diff(path, &content, &final_content);
    fs::write(&full, final_content).map_err(|e| e.to_string())?;
    Ok(format!("Replacement applied successfully to {} (lines {}-{})", path, start_line, end_line))
}

pub fn search_code(workspace: &str, sub_dir: &str, query: &str) -> Result<String, String> {
    let clean_workspace = workspace.strip_prefix(r"\\?\").unwrap_or(workspace);
    let clean_sub = sub_dir.trim().trim_matches(|c| c == '.' || c == '/' || c == '\\');

    let target = if clean_sub.is_empty() {
        PathBuf::from(clean_workspace)
    } else {
        secure_path(clean_workspace, sub_dir)?
    };

    if !target.exists() {
        return Err("Search target path does not exist".to_string());
    }

    let re = match Regex::new(query) {
        Ok(compiled) => compiled,
        Err(_) => {
            let escaped = regex::escape(query);
            Regex::new(&escaped).map_err(|e| format!("Search regex error: {}", e))?
        }
    };

    let walker = ignore::WalkBuilder::new(&target)
        .standard_filters(true)
        .hidden(true)
        .build();

    let mut matches = Vec::new();

    for entry in walker.flatten() {
        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            let path = entry.path();
            if let Ok(content) = fs::read_to_string(path) {
                let rel_path = path
                    .strip_prefix(clean_workspace)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");

                for (line_num, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        matches.push(format!("{}:{}: {}", rel_path, line_num + 1, line.trim()));
                        if matches.len() >= 150 {
                            matches.push("... [Showing first 150 matches. Narrow query or target specific directory to inspect more]".to_string());
                            return Ok(matches.join("\n"));
                        }
                    }
                }
            }
        }
    }

    if matches.is_empty() {
        return Ok("No matches found".to_string());
    }

    Ok(matches.join("\n"))
}