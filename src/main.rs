use anyhow::{anyhow, bail, Result};
use clap::Parser;
use dunce::canonicalize;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{stderr, stdout},
    path::{Path, PathBuf},
    process::Command,
};
use tracing::{error, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
struct Cli {
    dir: Option<PathBuf>,
    #[clap(short, long)]
    output: PathBuf,
    #[clap(short, long)]
    tmp: PathBuf,
}

#[non_exhaustive]
#[derive(Debug, Deserialize, Serialize)]
struct GrammarsFile {
    grammars: std::collections::HashMap<String, GrammarBuildInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GrammarSource {
    git: String,
    rev: String,
    subpath: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GrammarConfig {
    name: String,
    source: GrammarSource,
}

#[derive(Debug, Deserialize, Serialize)]
struct LanaugeConfig {
    grammar: Vec<GrammarConfig>,
}

#[non_exhaustive]
#[derive(Debug, Deserialize, Serialize)]
struct GrammarBuildInfo {
    git: String,
    rev: Option<String>,
    path: PathBuf,
    cpp: Option<bool>,
    relative: Option<PathBuf>,
    generate: Option<bool>,
}

fn logging() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

fn main() -> Result<()> {
    logging()?;

    let cli = Cli::parse();

    if !cli.output.exists() {
        if let Err(e) = std::fs::create_dir_all(&cli.output) {
            error!("Failed to create output dir: {}", e);
            bail!(e);
        };
    }
    let output_dir = match canonicalize(&cli.output) {
        Ok(v) => v,
        Err(e) => {
            bail!("Failed to canonicalize '{}': {e}", &cli.output.display());
        }
    };

    let tmp_dir = match canonicalize(&cli.tmp) {
        Ok(v) => v,
        Err(e) => {
            bail!("Failed to canonicalize '{}': {e}", &cli.tmp.display());
        }
    };

    let helix_dir = tmp_dir.join("helix");
    checkout_repo(
        &helix_dir,
        "https://github.com/helix-editor/helix",
        "0a4432b104099534f7a25b8ea4148234db146ab6",
    )?;

    let Ok(languages_config) = &fs::read_to_string(helix_dir.join("languages.toml"))
    else {
        error!("Failed to read grammars config");
        bail!("Failed to read grammars config");
    };
    let Ok(config) = toml::from_str::<LanaugeConfig>(languages_config) else {
        error!("Failed to deserialize config");
        bail!("Failed to deserialize config");
    };

    for grammar in config.grammar {
        if let Err(e) = build_grammar(&grammar, &output_dir, &tmp_dir) {
            println!("error build grammar {}: {e}", grammar.name,)
        }
    }
    Ok(())
}

fn checkout_repo(path: &Path, repo: &str, rev: &str) -> Result<()> {
    if path.join(".git").exists() {
        let output = Command::new("git")
            .current_dir(path)
            .arg("fetch")
            .output()?;
        if !output.status.success() {
            return Err(anyhow!("git fetch failed for {repo}"));
        }

        let _ = Command::new("git")
            .current_dir(path)
            .arg("checkout")
            .arg(rev)
            .output();
    } else {
        std::fs::create_dir_all(path)?;
        let output = Command::new("git")
            .current_dir(path)
            .arg("clone")
            .arg(repo)
            .arg(".")
            .output()?;
        if !output.status.success() {
            return Err(anyhow!("git clone failed for {repo}"));
        }

        let _ = Command::new("git")
            .current_dir(path)
            .arg("checkout")
            .arg(rev)
            .output();
    }

    Ok(())
}

fn build_grammar(
    grammar: &GrammarConfig,
    output_dir: &Path,
    tmp_dir: &Path,
) -> Result<()> {
    let path = tmp_dir.join(format! {"tree-sitter-{}",grammar.name});
    checkout_repo(&path, &grammar.source.git, &grammar.source.rev)?;
    let path = if let Some(subpath) = grammar.source.subpath.as_ref() {
        path.join(subpath)
    } else {
        path
    };
    build_tree_sitter(&grammar.name, &path, output_dir)?;
    Ok(())
}

fn build_tree_sitter(name: &str, path: &Path, output: &Path) -> Result<()> {
    println!("-----------------------------------");
    println!("now building tree sitter for {name}");
    let output = Command::new("tree-sitter")
        .current_dir(path)
        .arg("build")
        .arg("--output")
        .arg(output.join(format!(
            "libtree-sitter-{name}.{}",
            std::env::consts::DLL_EXTENSION
        )))
        .stdout(stdout())
        .stderr(stderr())
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("tree sitter build failed for {name}"));
    }
    Ok(())
}
