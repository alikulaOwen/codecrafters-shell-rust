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
    const BUILTIN_CMDS: [&str; 6] = ["echo", "type", "exit", "pwd", "cd", "history"];

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
        "cat" => {
            let mut output = String::new();
            let mut err_output = String::new();
            for file_path in args {
                let clean_path = file_path.trim_end_matches(&['\n', '\r'][..]);
                match std::fs::read_to_string(clean_path) {
                    Ok(content) => output.push_str(&content),
                    Err(_) => {
                        let msg = format!("cat: {}: No such file or directory\n", clean_path);
                        err_output.push_str(&msg);
                    }
                }
            }
            if !output.is_empty() {
                let _ = write!(stdout, "{}", output);
            }
            if !err_output.is_empty() {
                let _ = write!(stderr, "{}", err_output);
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
                    let _ = writeln!(stdout, "  {}  {}", i + 1, cmd);
                }
            }
            true
        }
        _ => false,
    }
}
