use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use dunce::canonicalize;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self},
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};
use tracing::{debug, error, info, Level};
use tracing_subscriber::FmtSubscriber;

const BUILD_TARGET: &str = env!("BUILD_TARGET");

#[derive(Parser)]
struct Cli {
    dir: Option<PathBuf>,
    #[clap(short, long)]
    output: PathBuf,
}

#[non_exhaustive]
#[derive(Debug, Deserialize, Serialize)]
struct GrammarsFile {
    grammars: std::collections::HashMap<String, GrammarBuildInfo>,
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

const GRAMMARS_CONFIG: &str = "config.toml";

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

    let Ok(grammars_config) = canonicalize(PathBuf::from(GRAMMARS_CONFIG)) else {
        error!("Failed to canonicalize grammars config");
        bail!("Failed to canonicalize grammars config");
    };
    let Ok(grammars) = &fs::read_to_string(grammars_config) else {
        error!("Failed to read grammars config");
        bail!("Failed to read grammars config");
    };
    let Ok(config) = toml::from_str::<GrammarsFile>(grammars) else {
        error!("Failed to deserialize config");
        bail!("Failed to deserialize config");
    };

    for (name, grammar) in config.grammars {
        info!("Building: {name}");

        if grammar.path.exists() {
            let output = Command::new("git")
                .current_dir(&grammar.path)
                .arg("fetch")
                .output()?;
            if !output.status.success() {
                return Err(anyhow!("git fetch failed"));
            }

            if let Some(rev) = &grammar.rev {
                let output = Command::new("git")
                    .current_dir(&grammar.path)
                    .arg("checkout")
                    .arg(rev)
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow!("git checkout failed"));
                }
            }
        } else {
            std::fs::create_dir_all(&grammar.path)?;
            let output = Command::new("git")
                .current_dir(&grammar.path)
                .arg("clone")
                .arg(&grammar.git)
                .arg(".")
                .output()?;
            if !output.status.success() {
                return Err(anyhow!("git clone failed"));
            }

            if let Some(rev) = &grammar.rev {
                let output = Command::new("git")
                    .current_dir(&grammar.path)
                    .arg("checkout")
                    .arg(rev)
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow!("git checkout failed"));
                }
            }
        }

        let grammar_path = match canonicalize(&grammar.path) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to canonicalize '{}': {e}", grammar.path.display());
                continue;
            }
        };
        let paths = TreeSitterPaths::new(
            grammar_path,
            grammar.relative,
            grammar.cpp,
            grammar.generate,
        );
        match build_tree_sitter_library(&paths, &output_dir, &name) {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to build grammar: {e}");
                continue;
            }
        };
    }

    Ok(())
}

fn build_tree_sitter_library(paths: &TreeSitterPaths, output: &Path, name: &str) -> Result<bool> {
    let mut library_path = output.join(name);
    library_path.set_extension(std::env::consts::DLL_EXTENSION);
    info!("Build object: {}", library_path.display());

    let should_recompile = paths.should_recompile(&library_path)?;

    if !should_recompile {
        return Ok(false);
    }

    let cpp = if let Some(TreeSitterScannerSource { path: _, cpp }) = paths.scanner {
        cpp
    } else {
        false
    };

    let mut compiler = cc::Build::new();
    compiler
        .cpp(cpp)
        .warnings(false)
        .include(&paths.source)
        .opt_level(3)
        .cargo_metadata(false)
        .shared_flag(true)
        .host(BUILD_TARGET)
        .target(BUILD_TARGET);

    let mut command = compiler.try_get_compiler()?.to_command();
    command.arg(&paths.parser);
    if cfg!(windows) {
        if let Some(TreeSitterScannerSource { ref path, .. }) = paths.scanner {
            command.arg(path);
        }
        command.arg("/link");
        command.arg("/DLL");
        command.arg(format!("/out:{}", library_path.to_str().unwrap()));
    } else {
        // command.arg(&paths.parser);
        command
            .arg("-fPIC")
            .arg("-fno-exceptions")
            .arg("-g")
            .arg("-o")
            .arg(&library_path);
        if let Some(TreeSitterScannerSource { ref path, cpp }) = paths.scanner {
            if cpp {
                command.arg("-xc++");
            } else {
                command.arg("-xc").arg("-std=c99");
            }
            command.arg(path);
        }
    }

    // Compile the tree sitter library
    let command_str = format!("{command:?}");
    debug!("Running {command_str}");
    let output = command
        .output()
        .with_context(|| format!("Failed to run C compiler. Command: {command_str}"))?;
    if !output.status.success() {
        bail!(
            "Parser compilation failed:\nCommand: {command_str}\nStdout: {}\nStderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(true)
}

struct TreeSitterScannerSource {
    path: PathBuf,
    cpp: bool,
}

struct TreeSitterPaths {
    source: PathBuf,
    parser: PathBuf,
    scanner: Option<TreeSitterScannerSource>,
}

impl TreeSitterPaths {
    fn new(
        repo: PathBuf,
        relative: Option<PathBuf>,
        cpp: Option<bool>,
        generate: Option<bool>,
    ) -> Self {
        let _cpp = cpp.unwrap_or_default();
        // Resolve subpath within the repo if any
        let subpath = relative.map(|subpath| repo.join(subpath)).unwrap_or(repo);

        if let Some(generate) = generate {
            if generate {
                match Command::new("tree-sitter")
                    .args(["generate", "--abi", "latest", "grammar.js"])
                    .current_dir(subpath.clone())
                    .status()
                {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to generate parser: {e}");
                    }
                };
            }
        }

        // Source directory
        let source = subpath.join("src");

        // Path to parser source
        let parser = source.join("parser.c");

        // Path to scanner if any
        let mut scanner_path = source.join("scanner.c");
        let scanner = if scanner_path.exists() {
            Some(TreeSitterScannerSource {
                path: scanner_path,
                cpp: false,
            })
        } else {
            scanner_path.set_extension("cc");
            if scanner_path.exists() {
                Some(TreeSitterScannerSource {
                    path: scanner_path,
                    cpp: true,
                })
            } else {
                None
            }
        };

        Self {
            source,
            parser,
            scanner,
        }
    }

    fn should_recompile(&self, library_path: &Path) -> Result<bool> {
        if !library_path.exists() {
            return Ok(true);
        };

        let library_mtime = mtime(library_path)?;
        if mtime(&self.parser)? > library_mtime {
            return Ok(true);
        }

        if let Some(TreeSitterScannerSource { ref path, .. }) = self.scanner {
            if mtime(path)? > library_mtime {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn mtime(path: &Path) -> Result<SystemTime> {
    let meta = match std::fs::metadata(path) {
        Ok(v) => v,
        Err(e) => bail!("Failed to get metadata: {e}"),
    };

    let modified = match meta.modified() {
        Ok(v) => v,
        Err(e) => bail!("Failed to get modified time: {e}"),
    };

    Ok(modified)
}
