use std::path::PathBuf;

pub fn load_env() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let _ = dotenvy::from_filename(manifest.join(".env"));
    dotenvy::dotenv().ok();
}

pub fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub struct McpPaths {
    pub worksheet: PathBuf,
    pub docx: PathBuf,
    pub slides: PathBuf,
}

pub fn mcp_paths() -> McpPaths {
    let m = manifest_dir();
    McpPaths {
        worksheet: m.join("../mcp-servers/worksheet-mcp/target/release/excel-mcp-server"),
        docx: m.join("../mcp-servers/docx-mcp/target/release/docx-mcp-server"),
        slides: m.join("../mcp-servers/mcp_slides/target/release/slides-mcp-server"),
    }
}

pub fn assert_mcp_binaries_exist(paths: &McpPaths) {
    for (name, path) in [
        ("worksheet", &paths.worksheet),
        ("docx", &paths.docx),
        ("slides", &paths.slides),
    ] {
        assert!(
            path.exists(),
            "MCP binary missing for {name}: {}\n\
             Build with: (cd mcp-servers/{name}-mcp && cargo build --release)",
            path.display(),
        );
    }
}

pub fn google_api_key() -> String {
    load_env();
    match std::env::var("GOOGLE_API_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => panic!("GOOGLE_API_KEY must be set in .env for validation tests"),
    }
}

pub fn gemini_model() -> String {
    load_env();
    std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3.1-flash-lite".into())
}