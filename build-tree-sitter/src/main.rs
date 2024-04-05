use anyhow::{bail, Context, Result};
use clap::Parser;
use dunce::canonicalize;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self},
    path::{Path, PathBuf},
    process::Command,
};
use tracing::{debug, error, info, Level};
use tracing_subscriber::FmtSubscriber;

const BUILD_TARGET: &str = env!("BUILD_TARGET");

#[derive(Parser)]
struct Cli {
    #[clap(short, long)]
    output: PathBuf,
}

#[non_exhaustive]
#[derive(Debug, Deserialize, Serialize)]
struct GrammarsFile {
    grammars: std::collections::HashMap<String, GrammarBuildInfo>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize)]
struct GrammarBuildInfo {
    path: PathBuf,
    src: Option<PathBuf>,
    relative: Option<PathBuf>,
    generate: Option<bool>,
    parser: ParserBuildInfo,
    scanner: ScannerBuildInfo,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ParserBuildInfo {
    path: PathBuf,
    cpp: bool,
    flags: Vec<String>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ScannerBuildInfo {
    path: PathBuf,
    cpp: bool,
    flags: Vec<String>,
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
    let config = toml::from_str::<GrammarsFile>(grammars)?;

    let cwd = std::env::current_dir()?;

    let build_dir = Path::new("./build");
    _ = std::fs::create_dir(build_dir);
    let build_dir = canonicalize(build_dir)?;

    for name in config.grammars.keys().sorted() {
        info!("Building: {name}");

        match build_tree_sitter_library(
            &build_dir,
            &output_dir,
            name,
            config.grammars[name].clone(),
        ) {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to build grammar '{name}': {e}");
                std::env::set_current_dir(cwd.clone())?;
                continue;
            }
        };

        std::env::set_current_dir(cwd.clone())?;
    }

    Ok(())
}

fn build_tree_sitter_library(
    build_dir: &Path,
    output: &Path,
    name: &str,
    gbi: GrammarBuildInfo,
) -> Result<bool> {
    let path = match canonicalize(gbi.path.clone()) {
        Ok(v) => v,
        Err(e) => bail!("Failed to canonicalize: {e}"),
    };

    let path = gbi
        .clone()
        .relative
        .clone()
        .map(|subpath| path.join(subpath))
        .unwrap_or(path);

    match std::env::set_current_dir(path.clone()) {
        Ok(_) => {}
        Err(e) => bail!("Failed to set_current_dir: {e}"),
    };

    info!(
        "Current dir: {}",
        match std::env::current_dir() {
            Ok(v) => v,
            Err(e) => bail!("Failed to set_current_dir: {e}"),
        }
        .display()
    );

    let library_path = build_dir.join(name);
    // library_path.set_extension(std::env::consts::DLL_EXTENSION);
    _ = std::fs::create_dir_all(library_path.clone());
    info!("Build object: {}", library_path.display());

    if gbi.generate.unwrap_or_default() {
        match Command::new("tree-sitter")
            .args(["generate", "--abi", "latest", "grammar.js"])
            .status()
        {
            Ok(_) => {}
            Err(e) => {
                bail!("Failed to generate parser: {e}");
            }
        };
    }

    let src = path.join(gbi.clone().src.unwrap_or("src".into()));
    std::env::set_current_dir(src.clone())?;

    // Scanner

    if gbi.scanner.path.exists() {
        let mut compiler = cc::Build::new();

        compiler
            .include(std::env::current_dir()?)
            .opt_level(3)
            .warnings(true)
            .cargo_metadata(false)
            .cargo_warnings(true)
            .cargo_debug(true)
            .cpp(gbi.parser.cpp)
            .host(BUILD_TARGET)
            .target(BUILD_TARGET)
            .out_dir(library_path.clone());

        compiler
            .flag_if_supported("-Wno-unused-parameter")
            .flag_if_supported("-Wno-unused-but-set-variable");

        for flag in gbi.scanner.flags.clone() {
            compiler.flag_if_supported(&flag);
        }

        compiler.shared_flag(false).static_flag(false);

        let scanner_path = src.join(gbi.scanner.path.clone());
        compiler.file(scanner_path);

        #[allow(clippy::useless_format)]
        compile(
            &mut compiler,
            format!("{}", library_path.clone().join("scanner.o").display()),
        )?;
    }

    // Parser

    let mut compiler = cc::Build::new();

    compiler
        .include(std::env::current_dir()?)
        .opt_level(3)
        .warnings(true)
        .cargo_metadata(false)
        .cargo_warnings(true)
        .cargo_debug(true)
        .cpp(gbi.parser.cpp)
        .host(BUILD_TARGET)
        .target(BUILD_TARGET);

    if library_path.clone().join("scanner.o").exists() {
        compiler.object(library_path.clone().join("scanner.o"));
    }

    compiler
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");

    compiler.std("c11");

    for flag in gbi.parser.flags.clone() {
        compiler.flag_if_supported(&flag);
    }

    compiler.shared_flag(true).static_flag(false);

    if !std::env::var("GITHUB_CI").unwrap_or_default().is_empty() {
        println!("::group::Build {name}");
    }

    let parser_path = src.join(gbi.parser.path.clone());
    compiler.file(parser_path);

    if !std::env::var("GITHUB_CI").unwrap_or_default().is_empty() {
        println!("::endgroup::");
    }

    let mut out_lib = output.join(format!("lib{name}"));
    out_lib.set_extension(std::env::consts::DLL_EXTENSION);
    compile(&mut compiler, out_lib.display().to_string())?;

    Ok(true)
}

fn compile(compiler: &mut cc::Build, out: String) -> Result<()> {
    let mut command = compiler.try_get_compiler()?.to_command();

    #[cfg(windows)]
    {
        command.arg("/link");
        command.arg("/DLL");
        command.arg(format!("/out:{out}"));
    }

    #[cfg(not(windows))]
    {
        command.arg("-fno-exceptions").arg("-g").arg("-o").arg(out);
    }

    command.arg("-c");

    for file in compiler.get_files() {
        command.arg(file.as_os_str());
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

    Ok(())
}
