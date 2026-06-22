use std::path::{Component, Path, PathBuf};

/// On-disk layout: `{root}/{user_id}/{session_id}/`
pub fn session_dir(root: &Path, user_id: &str, session_id: &str) -> PathBuf {
    root.join(user_id).join(session_id)
}

/// Public URL for a file inside a user-scoped session directory.
pub fn public_url(user_id: &str, session_id: &str, path: &Path, root: &Path) -> Option<String> {
    let base = session_dir(root, user_id, session_id);
    let rel = path.strip_prefix(&base).ok()?.to_str()?;
    if rel.is_empty() || rel.contains("..") {
        return None;
    }
    Some(format!("/artifacts/{user_id}/{session_id}/{rel}"))
}

/// Resolve a requested relative path safely inside `session_dir`.
pub fn resolve_file(session_root: &Path, relative: &str) -> Option<PathBuf> {
    let rel = Path::new(relative);
    if relative.is_empty() || relative.contains("..") {
        return None;
    }
    for component in rel.components() {
        if !matches!(component, Component::Normal(_)) {
            return None;
        }
    }
    let candidate = session_root.join(rel);
    let canonical_root = session_root.canonicalize().ok()?;
    let canonical_file = candidate.canonicalize().ok()?;
    if canonical_file.starts_with(&canonical_root) && canonical_file.is_file() {
        Some(canonical_file)
    } else {
        None
    }
}

pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}