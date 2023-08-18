use anyhow::{bail, Context, Result};
use colored::Colorize;
use log;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

fn main() {
    pretty_env_logger::init();
    build_grammar().unwrap();
}

fn build_grammar() -> Result<bool> {
    let grammar_dir = std::env::current_dir().unwrap();

    // Build the tree sitter library
    let paths = TreeSitterPaths::new(grammar_dir.clone(), None);
    build_tree_sitter_library(&paths).with_context(|| {
        format!(
            "Failed to build tree sitter library in {grammar_dir}",
            grammar_dir = grammar_dir.display()
        )
    })
}

fn tree_sitter_library_path() -> Result<PathBuf> {
    let mut library_path =
        std::env::current_dir()?.join(std::env::current_dir()?.file_name().unwrap());
    library_path.set_extension(std::env::consts::DLL_EXTENSION);
    Ok(library_path)
}

fn build_tree_sitter_library(paths: &TreeSitterPaths) -> Result<bool> {
    let library_path = tree_sitter_library_path()?;
    let should_recompile = paths.should_recompile(&library_path)?;

    if !should_recompile {
        return Ok(false);
    }

    log::info!(
        "{:>12} grammar {}",
        "Building".bold().bright_cyan(),
        library_path.to_string_lossy().dimmed(),
    );

    let cpp = if let Some(TreeSitterScannerSource { ref path, cpp }) = paths.scanner {
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
    log::info!("{:>12} {command_str}", "Running".bold().dimmed());
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

const BUILD_TARGET: &str = env!("BUILD_TARGET");
