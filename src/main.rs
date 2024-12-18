use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use clap::Parser;
use regex::Regex;
use shadow_rs::shadow;
use std::sync::LazyLock;
use tracing::{info, warn};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\w+)\(([^\)]+)\)\s+= (\S+)(.*)").unwrap());

const ABOUT: &str =
    "Copy the files needed for a program, from one prefix to another, using strace.";
const LONG_ABOUT: &str =
    "Copy the files needed for a program, from one prefix to another, using strace.

To create the required strace log file run (see man strace for details):

strace -o <log file> -ff -e trace=file,process <command line ...>
";

shadow!(build);

#[derive(Parser, Debug)]
#[command(
    name="strace-copy",
    version = build::CLAP_LONG_VERSION,
    author="Jérôme Robert",
    about=ABOUT,
    long_about=LONG_ABOUT
)]
struct Cli {
    #[arg(long, short)]
    verbose: bool,
    #[arg(long, default_value = "/usr/")]
    /// Source prefix
    prefix: PathBuf,
    /// Destination prefix
    destination_prefix: PathBuf,
    #[arg(num_args=1..)]
    /// input `strace` log files
    strace_logs: Vec<String>,
}

fn strace_line_to_path(line: &str) -> Option<PathBuf> {
    let caps = RE.captures(line)?;
    let name = caps[1].to_string();
    if name.starts_with("syscall") || name == "exit" || name == "exit_group" {
        return None;
    }
    let args: Vec<String> = caps[2]
        .split(',')
        .map(|s| s.trim_matches(['"', ' ']).to_string())
        .collect();
    let Ok(return_value) = caps[3].parse::<isize>() else {
        warn!("Skipping line {line}");
        return None;
    };
    if return_value == -1 {
        None
    } else {
        match name.as_str() {
            "openat" | "newfstatat" => Some(args[1].clone().into()),
            "open" | "readlink" | "execve" => Some(args[0].clone().into()),
            _ => None,
        }
    }
}

fn usrmerge(path: &Path) -> PathBuf {
    path.strip_prefix("/lib").map_or_else(
        |_| path.into(),
        |path| PathBuf::from(&"/usr/lib").join(path),
    )
}

fn init_logger(verbose: bool) {
    let default_log_level = if verbose {
        LevelFilter::INFO
    } else {
        LevelFilter::WARN
    };
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(default_log_level.into())
                .from_env_lossy(),
        )
        .init();
}

fn relative_path(a: &Path, b: &Path) -> PathBuf {
    let mut a_iter = a.iter();
    let mut b_iter = b.iter();

    // Find the common ancestor
    while let (Some(a_comp), Some(b_comp)) = (a_iter.next(), b_iter.next()) {
        if a_comp != b_comp {
            let mut relative_path = PathBuf::new();
            for _ in a_iter {
                relative_path.push("..");
            }
            relative_path.push(b_comp);
            relative_path.extend(b_iter);
            return relative_path;
        }
    }
    panic!("Cannot compute relative path of identical path: {a:?} {b:?}");
}

fn main() {
    let cli = Cli::parse();
    init_logger(cli.verbose);
    for input_file in cli.strace_logs {
        let file = File::open(input_file).expect("Cannot open file");
        let reader = BufReader::new(file);
        for src_path in reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|x| strace_line_to_path(&x))
            .filter(|x| x.is_file())
            .map(|x| usrmerge(&x))
        {
            let can_path = match std::fs::canonicalize(&src_path) {
                Ok(x) => x,
                Err(e) => {
                    warn!("Cannot canonicalize {src_path:?}: {e:?}");
                    continue;
                }
            };
            if let Ok(path) = can_path.strip_prefix(&cli.prefix) {
                let dst_path = cli.destination_prefix.join(path);
                if let Some(parent) = dst_path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        warn!("Cannot create directory {parent:?}: {e:?}");
                    }
                }
                info!("Copying {src_path:?} to {dst_path:?}");
                if let Err(e) = std::fs::copy(&src_path, &dst_path) {
                    warn!("{e:?}");
                }
                // FIXME: we currently only support path which are symlinks but not path whose one
                // parent is a symlink
                if can_path != src_path && src_path.is_symlink() {
                    if let Ok(nc_path) = src_path.strip_prefix(&cli.prefix) {
                        // create a symlink from nc_path to path.
                        let link = cli.destination_prefix.join(nc_path);
                        let _ = std::fs::remove_file(&link);
                        let original = cli.destination_prefix.join(path);
                        let rel_sl_tgt = relative_path(&link, &original);
                        info!("Create link {link:?} to {rel_sl_tgt:?} (aka {original:?})");
                        if let Some(parent) = link.parent() {
                            if let Err(e) = std::fs::create_dir_all(parent) {
                                warn!("Cannot create directory {parent:?}: {e:?}");
                            }
                        }
                        std::os::unix::fs::symlink(&rel_sl_tgt, &link).unwrap_or_else(|_| {
                            panic!("Cannot create symlink from {link:?} to {original:?}")
                        });
                    }
                }
            }
        }
    }
}
