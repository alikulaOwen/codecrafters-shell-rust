/*
 * Thought Process:
 * - This module handles internal shell commands (builtins) like 'echo', 'cd', 'exit', 'pwd', 'type',
 *   'cat', and 'history'. Because they run in the shell's process context (especially 'cd' and 'exit'),
 *   they cannot be external processes and must be executed directly by mutating shell state (e.g. 'env::set_current_dir').
 *
 * External Packages / Crates Alternative:
 * - 'clap' (Command Line Argument Parser) or 'lexopt' can parse arguments, flags, and options for builtins.
 *
 * Why External Packages are Easier:
 * - Writing custom argument parsers (like handling history flags '-a', '-w', '-r' manually) is error-prone.
 *   Crates like 'clap' automatically validate types, support nested subcommands, format '--help' text perfectly,
 *   and enforce standard CLI conventions out of the box.
 */

use std::env;
use std::process::exit;
use std::io::Write;
use pathsearch::find_executable_in_path;

pub fn run_builtin(
    command: &str,
    args: &[&str],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    history: Option<&[String]>,
) -> bool {
    const BUILTIN_CMDS: [&str; 7] = ["echo", "type", "exit", "pwd", "cd", "history", "jobs"];

    match command {
        "exit" => exit(0),
        "echo" => {
            let output = args.join(" ");
            let _ = writeln!(stdout, "{}", output);
            true
        }
        "type" => {
            let phrase = args.get(0).unwrap_or(&"");
            if BUILTIN_CMDS.contains(phrase) {
                let _ = writeln!(stdout, "{} is a shell builtin", phrase);
            } else if let Some(exe) = find_executable_in_path(phrase) {
                let _ = writeln!(stdout, "{} is {}", phrase, exe.display());
            } else {
                let _ = writeln!(stdout, "{}: not found", phrase);
            }
            true
        }
        "pwd" => {
            if let Ok(current_dir) = env::current_dir() {
                let _ = writeln!(stdout, "{}", current_dir.display());
            }
            true
        }
        "cd" => {
            let new_dir = args.get(0).unwrap_or(&"");
            let path = if new_dir.is_empty() {
                env::var("HOME").unwrap_or_default()
            } else if new_dir.starts_with('~') {
                let home_dir = env::var("HOME").unwrap_or_default();
                new_dir.replacen('~', &home_dir, 1)
            } else {
                new_dir.to_string()
            };
            
            if let Err(_) = env::set_current_dir(&path) {
                let _ = writeln!(stderr, "cd: {}: No such file or directory", path);
            }
            true
        }

        "history" => {
            if let Some(hist) = history {
                let limit = if let Some(n_str) = args.get(0) {
                    n_str.parse::<usize>().ok()
                } else {
                    None
                };
                
                // Determine which entries to show
                let start_index = if let Some(n) = limit {
                    if n < hist.len() {
                        hist.len() - n
                    } else {
                        0
                    }
                } else {
                    0
                };
                
                // Display history entries
                for (i, cmd) in hist.iter().enumerate().skip(start_index) {
                    let _ = writeln!(stdout, "{:>5}  {}", i + 1, cmd);
                }
            }
            true
        }
        _ => false,
    }
}
