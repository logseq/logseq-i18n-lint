mod analyzer;
mod checker;
mod config;
mod edn;
mod git;
mod key_collector;
mod parser;
mod reporter;
mod scanner;

use std::path::{Path, PathBuf};
use std::process;

use clap::{Parser, Subcommand};
use colored::Colorize;

use config::AppConfig;
use reporter::OutputFormat;

/// Default config file path looked up relative to the current working directory.
const DEFAULT_CONFIG_PATH: &str = ".i18n-lint.toml";

#[derive(Parser)]
#[command(name = "logseq-i18n-lint", version, about = "AST-level detection of hardcoded UI strings in Clojure/ClojureScript")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = ".i18n-lint.toml")]
    config: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Detect hardcoded UI strings
    Lint {
        /// Output format: table or compact
        #[arg(short, long, default_value = "table")]
        format: OutputFormat,

        /// Warn only, do not exit with error code
        #[arg(short, long)]
        warn_only: bool,

        /// Only check git changed files
        #[arg(short, long)]
        git_changed: bool,
    },
    /// Check for unused translation keys in dictionary files
    CheckKeys {
        /// Remove unused keys from all dictionary files
        #[arg(long)]
        fix: bool,
    },
}

fn resolve_base_dir(config: &AppConfig) -> PathBuf {
    // The project root is always resolved relative to the executable's directory,
    // making behaviour independent of the working directory from which the binary
    // is invoked.  An empty project_root means the executable resides at the
    // project root itself.
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}: cannot determine executable path: {e}", "error".red().bold());
            process::exit(2);
        }
    };
    let exe_dir = exe_path.parent().unwrap_or(Path::new("."));
    let candidate = if config.project_root.is_empty() {
        exe_dir.to_path_buf()
    } else {
        exe_dir.join(&config.project_root)
    };
    match candidate.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "{}: cannot resolve project root '{}' relative to '{}': {e}",
                "error".red().bold(),
                if config.project_root.is_empty() { "." } else { &config.project_root },
                exe_dir.display()
            );
            process::exit(2);
        }
    }
}

fn run_lint(config: &AppConfig, base_dir: &Path, format: OutputFormat, warn_only: bool, git_changed: bool, verbose: bool) {
    if let Err(msg) = config.validate_for_lint() {
        eprintln!("{}: {msg}", "error".red().bold());
        process::exit(2);
    }

    let files = if git_changed {
        match git::changed_files(config, base_dir) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{}: failed to get git changed files: {e}", "error".red().bold());
                process::exit(2);
            }
        }
    } else {
        match scanner::scan_files(
            &scanner::ScanConfig {
                include_dirs: &config.include_dirs,
                exclude_patterns: &config.lint.exclude_patterns,
                file_extensions: &config.file_extensions,
            },
            base_dir,
        ) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{}: failed to scan files: {e}", "error".red().bold());
                process::exit(2);
            }
        }
    };

    if verbose {
        eprintln!("{}: found {} files to check", "info".cyan(), files.len());
    }

    let file_count = files.len();
    let diagnostics = analyzer::analyze_files(&files, config);

    if diagnostics.is_empty() {
        println!("{}", "No hardcoded strings found.".green());
        process::exit(0);
    }

    reporter::report(&diagnostics, format, config, file_count, base_dir);

    if warn_only {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

fn run_check_keys(config: &AppConfig, base_dir: &Path, fix: bool, verbose: bool) {
    if let Err(msg) = config.validate_for_check_keys() {
        eprintln!("{}: {msg}", "error".red().bold());
        process::exit(2);
    }

    if verbose {
        eprintln!("{}: checking unused keys in {}", "info".cyan(), config.check_keys.primary_dict);
    }

    let result = match checker::check_unused_keys(config, base_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}: {e}", "error".red().bold());
            process::exit(2);
        }
    };

    if verbose {
        eprintln!(
            "{}: {} defined, {} referenced",
            "info".cyan(),
            result.total_defined,
            result.total_referenced,
        );
    }

    if result.unused_keys.is_empty() {
        println!("{}", "All translation keys are used.".green());
        process::exit(0);
    }

    println!(
        "\n{} unused translation key(s) found:\n",
        result.unused_keys.len()
    );
    // Print as table
    let max_width = result.unused_keys.iter().map(String::len).max().unwrap_or(20);
    let col_width = max_width.max(10);
    println!("| {:col_width$} |", "unused-key");
    println!("|-{}-|", "-".repeat(col_width));
    for key in &result.unused_keys {
        println!("| {key:>col_width$} |");
    }
    println!();

    if fix {
        match checker::fix_unused_keys(config, base_dir, &result.unused_keys) {
            Ok(count) => {
                println!(
                    "{}: removed {} unused key(s) from {} dictionary file(s).",
                    "fixed".green().bold(),
                    result.unused_keys.len(),
                    count,
                );
                process::exit(0);
            }
            Err(e) => {
                eprintln!("{}: failed to fix: {e}", "error".red().bold());
                process::exit(2);
            }
        }
    }

    process::exit(1);
}

fn main() {
    let cli = Cli::parse();

    // If the user explicitly provided -c/--config and the file does not exist,
    // fail immediately rather than silently falling back to built-in defaults.
    if cli.config != DEFAULT_CONFIG_PATH && !std::path::Path::new(&cli.config).exists() {
        eprintln!(
            "{}: config file not found: {}",
            "error".red().bold(),
            cli.config
        );
        process::exit(2);
    }

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
    }

    let base_dir = resolve_base_dir(&config);

    if cli.verbose {
        eprintln!("{}: analysis base directory: {}", "info".cyan(), base_dir.display());
    }

    match cli.command {
        Commands::Lint { format, warn_only, git_changed } => {
            run_lint(&config, &base_dir, format, warn_only, git_changed, cli.verbose);
        }
        Commands::CheckKeys { fix } => {
            run_check_keys(&config, &base_dir, fix, cli.verbose);
        }
    }
}
