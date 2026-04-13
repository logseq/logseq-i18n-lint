mod analyzer;
mod config;
mod git;
mod parser;
mod reporter;
mod scanner;

use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use colored::Colorize;

use config::AppConfig;
use reporter::OutputFormat;

#[derive(Parser)]
#[command(name = "logseq-i18n-lint", version, about = "AST-level detection of hardcoded UI strings in Clojure/ClojureScript")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = ".i18n-lint.toml")]
    config: String,

    /// Output format: table or compact
    #[arg(short, long, default_value = "table")]
    format: OutputFormat,

    /// Warn only, do not exit with error code
    #[arg(short, long)]
    warn_only: bool,

    /// Only check git changed files
    #[arg(short, long)]
    git_changed: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();

    let config = match AppConfig::load(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: failed to load config: {e}", "error".red().bold());
            process::exit(2);
        }
    };

    if cli.verbose {
        eprintln!(
            "{}: loaded config from {}",
            "info".cyan(),
            cli.config
        );
        eprintln!(
            "{}: scanning {} directories",
            "info".cyan(),
            config.include_dirs.len()
        );
    }

    // Compute the analysis base directory.
    // If project_root is set in config, resolve it relative to the executable's directory.
    // Otherwise fall back to the current working directory.
    let base_dir: PathBuf = if config.project_root.is_empty() {
        match std::env::current_dir() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}: cannot determine working directory: {e}", "error".red().bold());
                process::exit(2);
            }
        }
    } else {
        let exe_path = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}: cannot determine executable path: {e}", "error".red().bold());
                process::exit(2);
            }
        };
        let exe_dir = exe_path.parent().unwrap_or(Path::new("."));
        let candidate = exe_dir.join(&config.project_root);
        match candidate.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "{}: cannot resolve project_root '{}' relative to '{}': {e}",
                    "error".red().bold(),
                    config.project_root,
                    exe_dir.display()
                );
                process::exit(2);
            }
        }
    };

    if cli.verbose {
        eprintln!("{}: analysis base directory: {}", "info".cyan(), base_dir.display());
    }

    let files = if cli.git_changed {
        match git::changed_files(&config, &base_dir) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{}: failed to get git changed files: {e}", "error".red().bold());
                process::exit(2);
            }
        }
    } else {
        match scanner::scan_files(&config, &base_dir) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{}: failed to scan files: {e}", "error".red().bold());
                process::exit(2);
            }
        }
    };

    if cli.verbose {
        eprintln!("{}: found {} files to check", "info".cyan(), files.len());
    }

    let file_count = files.len();
    let diagnostics = analyzer::analyze_files(&files, &config);

    if diagnostics.is_empty() {
        println!("{}", "No hardcoded strings found.".green());
        process::exit(0);
    }

    reporter::report(&diagnostics, cli.format, &config, file_count, &base_dir);

    if cli.warn_only {
        process::exit(0);
    } else {
        process::exit(1);
    }
}
