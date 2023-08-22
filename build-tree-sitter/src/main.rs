use anyhow::{bail, Context, Result};
use clap::Parser;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};
use tracing::{info, Level, error};
use tracing_subscriber::FmtSubscriber;

const BUILD_TARGET: &str = env!("BUILD_TARGET");

#[derive(Parser)]
struct Cli {
    dir: PathBuf,
    output: PathBuf,
}

fn logging() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

fn find(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    info!("Walking over {}", dir.display());

    let mut paths = vec![];
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let f_name = entry.file_name().to_string_lossy();

        if f_name == "src" {
            let path = entry.into_path().parent().unwrap().to_path_buf();
            info!("Found {}", path.display());
            paths.push(path);
        }
    }

    Ok(paths)
}

fn main() -> Result<()> {
    logging()?;

    let cli = Cli::parse();
    let grammars_dir = &cli.dir.canonicalize()?;
    let output_dir = cli.output.canonicalize()?;
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)?;
    }

    let paths = find(grammars_dir)?;
    for grammar_dir in paths {
        let paths = TreeSitterPaths::new(grammar_dir.clone(), None);
        match build_tree_sitter_library(&paths, &output_dir, grammar_dir.file_name().unwrap().to_str().unwrap()) {
            Ok(_) => {},
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

    info!("Building grammar {}", library_path.display());

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
    info!("Running {command_str}");
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
    fn new(repo: PathBuf, relative: Option<PathBuf>) -> Self {
        // Resolve subpath within the repo if any
        let subpath = relative.map(|subpath| repo.join(subpath)).unwrap_or(repo);

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
        let mtime = |path| mtime(path).context("Failed to compare source and library timestamps");
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
    Ok(std::fs::metadata(path)?.modified()?)
}
