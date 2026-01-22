use crate::memory::{Memory, GraphNode};
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
use rayon::prelude::*;

pub mod parser;
pub mod registry;
pub mod scanner;
pub mod extractor;

use scanner::{Scanner, FileValue};
use extractor::Extractor;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum LibraryType {
    Rust,
    Node,
    Python,
}

pub struct DetectedLibrary {
    pub name: String,
    pub version: String,
    pub lib_type: LibraryType,
}

// --- Pure Functions & Data Transformation ---

pub async fn ingest_file(memory: &Memory, path: &Path) -> Result<()> {
    ingest_batch(memory, &[path.to_path_buf()]).await
}

pub async fn ingest_batch(memory: &Memory, paths: &[PathBuf]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    // 1. Parallel Scan (IO-bound but sync)
    let files: Vec<FileValue> = paths.par_iter()
        .filter_map(|p| Scanner::scan_file(p).ok().flatten())
        .collect();

    // 2. Sequential/Async Filter (DB-bound)
    let mut candidates = Vec::new();
    for file in files {
        if should_reindex(memory, &file).await? {
            candidates.push(file);
        }
    }

    if candidates.is_empty() {
        return Ok(());
    }

    println!("📝 Re-indexing {} changed files in parallel...", candidates.len());

    // 3. Parallel Extraction (CPU-bound)
    let all_nodes_and_files: Vec<(Vec<GraphNode>, FileValue)> = candidates.into_par_iter()
        .map(|file| {
            let nodes = Extractor::extract_symbols(&file);
            (nodes, file)
        })
        .collect();

    // 4. Batch Commit (Side effects)
    for (nodes, file) in all_nodes_and_files {
        commit_nodes(memory, nodes, &file).await?;
    }
    
    Ok(())
}

async fn should_reindex(memory: &Memory, file: &FileValue) -> Result<bool> {
    let path_str = file.path.to_str().unwrap_or_default();
    if let Ok(Some((_, old_hash))) = memory.check_sync_status(path_str).await {
        if old_hash == file.hash {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn commit_nodes(memory: &Memory, nodes: Vec<GraphNode>, file: &FileValue) -> Result<()> {
    let path_str = file.path.to_str().unwrap_or_default();
    if !nodes.is_empty() {
        memory.batch_add_nodes(nodes).await?;
    }
    memory.update_sync_status(path_str, &file.hash).await?;
    Ok(())
}

// --- Library Scanning (Pure IO) ---

pub fn scan_all_dependencies() -> Result<Vec<DetectedLibrary>> {
    let mut libs = Vec::new();
    if let Ok(rust_libs) = scan_rust_dependencies() { libs.extend(rust_libs); }
    if let Ok(node_libs) = scan_node_dependencies() { libs.extend(node_libs); }
    if let Ok(py_libs) = scan_python_dependencies() { libs.extend(py_libs); }
    libs.sort_by(|a, b| a.name.cmp(&b.name));
    libs.dedup_by(|a, b| a.name == b.name && a.lib_type == b.lib_type);
    Ok(libs)
}

pub fn scan_rust_dependencies() -> Result<Vec<DetectedLibrary>> {
    let path = Path::new("Cargo.toml");
    if !path.exists() { return Ok(vec![]); }
    let content = fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&content)?;
    let mut libs = Vec::new();
    let mut process_table = |table: &toml::value::Table| {
        for (name, val) in table {
            let version = match val {
                toml::Value::String(s) => s.clone(),
                toml::Value::Table(t) => t.get("version").and_then(|v| v.as_str()).unwrap_or("latest").to_string(),
                _ => "latest".to_string(),
            };
            libs.push(DetectedLibrary { name: name.clone(), version, lib_type: LibraryType::Rust });
        }
    };
    if let Some(deps) = value.get("dependencies").and_then(|d| d.as_table()) { process_table(deps); }
    if let Some(workspace) = value.get("workspace").and_then(|w| w.as_table()) {
        if let Some(deps) = workspace.get("dependencies").and_then(|d| d.as_table()) { process_table(deps); }
    }
    Ok(libs)
}

pub fn scan_node_dependencies() -> Result<Vec<DetectedLibrary>> {
    let path = Path::new("package.json");
    if !path.exists() { return Ok(vec![]); }
    let content = fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&content)?;
    let mut libs = Vec::new();
    if let Some(deps) = v.get("dependencies").and_then(|d| d.as_object()) {
        for (name, ver) in deps {
            libs.push(DetectedLibrary {
                name: name.clone(),
                version: ver.as_str().unwrap_or("latest").to_string(),
                lib_type: LibraryType::Node,
            });
        }
    }
    Ok(libs)
}

pub fn scan_python_dependencies() -> Result<Vec<DetectedLibrary>> {
    let mut libs = Vec::new();
    let req_path = Path::new("requirements.txt");
    if req_path.exists() {
        if let Ok(content) = fs::read_to_string(req_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                let parts: Vec<&str> = line.split(&['=', '>', '<', '~'][..]).collect();
                if !parts[0].trim().is_empty() {
                    libs.push(DetectedLibrary {
                        name: parts[0].trim().to_string(),
                        version: if parts.len() > 1 { parts[1].trim().to_string() } else { "latest".to_string() },
                        lib_type: LibraryType::Python,
                    });
                }
            }
        }
    }
    Ok(libs)
}

// --- Documentation Sync (IO + DB) ---

pub async fn sync_libraries(memory: &Memory, client: &reqwest::Client, libs: &[DetectedLibrary]) -> Result<()> {
    let mut entries = Vec::new();
    for lib in libs {
        println!("     Processing {}...", lib.name);
        let id = format!("{}_{}", lib.name, lib.version);
        
        let doc_content = match fetch_documentation(client, lib).await {
            Ok(doc) => doc,
            Err(_) => String::new(),
        };

        let embedding = if !doc_content.is_empty() {
            memory.embed(&doc_content).unwrap_or(vec![0.0; 384])
        } else {
            vec![0.0; 384]
        };
        
        let chunk_type = if doc_content.is_empty() { "metadata" } else { "documentation" };
        let content = if doc_content.is_empty() { format!("Library: {} {}", lib.name, lib.version) } else { doc_content };

        entries.push((id, lib.name.clone(), lib.version.clone(), content, String::new(), chunk_type.to_string(), embedding));
    }
    if !entries.is_empty() { memory.batch_add_library_entries(entries).await?; }
    Ok(())
}

async fn fetch_documentation(client: &reqwest::Client, lib: &DetectedLibrary) -> Result<String> {
    match lib.lib_type {
        LibraryType::Rust => fetch_rust_docs(client, lib).await,
        LibraryType::Node => fetch_node_docs(client, lib).await,
        LibraryType::Python => fetch_python_docs(client, lib).await,
    }
}

async fn fetch_node_docs(client: &reqwest::Client, lib: &DetectedLibrary) -> Result<String> {
    let url = format!("https://registry.npmjs.org/{}", lib.name);
    let res = client.get(&url).send().await?;
    if res.status().is_success() {
        let val: serde_json::Value = res.json().await?;
        if let Some(readme) = val["readme"].as_str() {
            return Ok(readme.to_string());
        }
    }
    Err(anyhow!("No README found in NPM registry"))
}

async fn fetch_python_docs(client: &reqwest::Client, lib: &DetectedLibrary) -> Result<String> {
    let url = format!("https://pypi.org/pypi/{}/json", lib.name);
    let res = client.get(&url).send().await?;
    if res.status().is_success() {
        let val: serde_json::Value = res.json().await?;
        if let Some(desc) = val["info"]["description"].as_str() {
            return Ok(desc.to_string());
        }
    }
    Err(anyhow!("No description found in PyPI"))
}

async fn fetch_rust_docs(client: &reqwest::Client, lib: &DetectedLibrary) -> Result<String> {
    let url = format!("https://crates.io/api/v1/crates/{}", lib.name);
    let res = client.get(&url).header("User-Agent", "Sly-Bot (github.com/sly)").send().await?;
    
    let mut repo_url = String::new();
    if res.status().is_success() {
         let val: serde_json::Value = res.json().await?;
         if let Some(repo) = val["crate"]["repository"].as_str() {
             repo_url = repo.to_string();
         }
    }

    if !repo_url.is_empty() {
        if repo_url.contains("github.com") {
            let raw_base = repo_url.replace("github.com", "raw.githubusercontent.com");
            let readme_url = format!("{}/HEAD/README.md", raw_base);
            let readme_res = client.get(&readme_url).send().await?;
            if readme_res.status().is_success() {
                return Ok(readme_res.text().await?);
            }
            let readme_url_2 = format!("{}/HEAD/README.markdown", raw_base);
            let readme_res_2 = client.get(&readme_url_2).send().await?;
            if readme_res_2.status().is_success() {
                return Ok(readme_res_2.text().await?);
            }
        }
    }

    Ok(format!("Crate: {}. Documentation available at https://docs.rs/{}", lib.name, lib.name))
}

// --- Skill Bootstrapping (Data -> DB) ---

pub async fn ensure_skills_loaded(memory: &Memory) -> Result<()> {
    let skills = vec![
        ("sly_sum",
         r#"(module (func (export "run") (param i32 i32) (result i32) local.get 0 local.get 1 i32.add))"#,
         "Adds two integers", "(i32, i32) -> i32"),
        ("sly_max",
         r#"(module (func (export "run") (param i32 i32) (result i32) local.get 0 local.get 1 local.get 0 local.get 1 i32.ge_s select))"#,
         "Returns the larger of two integers", "(i32, i32) -> i32"),
        ("sly_min",
         r#"(module (func (export "run") (param i32 i32) (result i32) local.get 0 local.get 1 local.get 0 local.get 1 i32.le_s select))"#,
         "Returns the smaller of two integers", "(i32, i32) -> i32"),
        ("sly_mul",
         r#"(module (func (export "run") (param i32 i32) (result i32) local.get 0 local.get 1 i32.mul))"#,
         "Multiplies two integers", "(i32, i32) -> i32"),
        ("sly_parity",
         r#"(module (func (export "run") (param i32) (result i32) local.get 0 i32.const 2 i32.rem_s i32.eqz))"#,
         "Returns 1 if even, 0 if odd", "(i32) -> i32"),
    ];

    for (name, code, desc, sig) in skills {
        memory.register_skill(name, code, desc, sig).await?;
    }
    Ok(())
}
