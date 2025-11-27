use pathsearch::find_executable_in_path;
use rustyline::completion::{Completer, Pair};
use rustyline::Context;
use rustyline::Editor;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::{exit, Command};

struct BuiltinCompleter;
use rustyline::Helper;
use rustyline::hint::Hinter;
use rustyline::highlight::Highlighter;
use rustyline::validate::Validator;

impl Helper for BuiltinCompleter {}
impl Hinter for BuiltinCompleter {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}
impl Highlighter for BuiltinCompleter {}
impl Validator for BuiltinCompleter {}
impl Completer for BuiltinCompleter {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), rustyline::error::ReadlineError> {
        let candidates = ["echo", "exit"];
        let fragment = &line[..pos];
        let mut matches = Vec::new();
        for &cmd in &candidates {
            if cmd.starts_with(fragment) {
                matches.push(Pair {
                    display: format!("{} ", cmd),
                    replacement: format!("{} ", cmd),
                });
            }
        }
        Ok((0, matches))
    }
}

fn parse_input(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_single_quotes = false;
    let mut in_double_quotes = false;

    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quotes => {
                in_single_quotes = !in_single_quotes;
            }
            '"' if !in_single_quotes => {
                in_double_quotes = !in_double_quotes;
            }
            '\\' if in_double_quotes => {
                if let Some(&next) = chars.peek() {
                    match next {
                        '"' | '\\' => {
                            chars.next();
                            current_token.push(next);
                        }
                        _ => {
                            current_token.push('\\');
                            chars.next();
                            current_token.push(next);
                        }
                    }
                } else {
                    current_token.push('\\');
                }
            }
            '\\' if !in_single_quotes && !in_double_quotes => {
                if let Some(next) = chars.next() {
                    current_token.push(next);
                }
            }
            ' ' | '\t' if !in_single_quotes && !in_double_quotes => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
            }
            _ => {
                current_token.push(c);
            }
        }
    }

    if !current_token.is_empty() {
        tokens.push(current_token);
    }

    for token in tokens.iter_mut() {
        while token.ends_with('\n') || token.ends_with('\r') {
            token.pop();
        }
    }

    tokens
}

fn main() {
    let mut rl = Editor::<BuiltinCompleter, rustyline::history::DefaultHistory>::new().unwrap();
    // Completer is provided via Helper in rustyline v12
    loop {
        let input = rl.readline("$ ");
        match input {
            Ok(line) => {
                const BUILTIN_CMDS: [&str; 5] = ["echo", "type", "exit", "pwd", "cd"];
                let mut tokens = parse_input(&line);

                // Handle output redirection tokens: ">" and "1>"
                let mut redirect_path: Option<String> = None;
                // Handle append stdout tokens: ">>" and "1>>"
                let mut append_redirect_path: Option<String> = None;
                // Handle stderr redirection token: "2>"
                let mut stderr_redirect_path: Option<String> = None;
                // Handle append stderr token: "2>>"
                let mut stderr_append_path: Option<String> = None;
                if let Some(pos) = tokens.iter().position(|t| t == ">" || t == "1>") {
                    if pos + 1 < tokens.len() {
                        redirect_path = Some(tokens[pos + 1].clone());
                        tokens.remove(pos + 1);
                        tokens.remove(pos);
                    }
                }
                if let Some(pos) = tokens.iter().position(|t| t == ">>" || t == "1>>") {
                    if pos + 1 < tokens.len() {
                        append_redirect_path = Some(tokens[pos + 1].clone());
                        tokens.remove(pos + 1);
                        tokens.remove(pos);
                    }
                }
                if let Some(pos) = tokens.iter().position(|t| t == "2>") {
                    if pos + 1 < tokens.len() {
                        stderr_redirect_path = Some(tokens[pos + 1].clone());
                        tokens.remove(pos + 1);
                        tokens.remove(pos);
                    }
                }
                if let Some(pos) = tokens.iter().position(|t| t == "2>>") {
                    if pos + 1 < tokens.len() {
                        stderr_append_path = Some(tokens[pos + 1].clone());
                        tokens.remove(pos + 1);
                        tokens.remove(pos);
                    }
                }

                if tokens.is_empty() {
                    continue;
                }

                let command = &tokens[0];
                let args: Vec<&str> = tokens[1..].iter().map(|s| s.as_str()).collect();
                match command.as_str() {
                    "exit" => exit(0),
                    "echo" => {
                        let output_str = args.join(" ");
                        if let Some(path) = redirect_path {
                            // Echo traditionally appends a trailing newline
                            let _ = std::fs::write(path, format!("{}\n", output_str));
                        } else if let Some(path) = append_redirect_path {
                            use std::io::Write as _;
                            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                                let _ = writeln!(file, "{}", output_str);
                            }
                        } else {
                            println!("{}", output_str);
                        }
                        // If stderr is redirected, ensure target file exists even though echo emits no stderr
                        if let Some(path) = stderr_redirect_path.clone() {
                            let _ = std::fs::write(path, "");
                        }
                    }
                    "type" => {
                        let phrase = args.get(0).unwrap_or(&"");
                        if BUILTIN_CMDS.contains(phrase) {
                            println!("{} is a shell builtin", phrase);
                        } else if let Some(exe) = find_executable_in_path(phrase) {
                            println!("{} is {}", phrase, exe.display());
                        } else {
                            let path_var = env::var("PATH").unwrap_or_default();
                            let paths = env::split_paths(&path_var);
                            let mut found = false;
                            for path in paths {
                                let exe_path = path.join(phrase);
                if let Some(path) = stderr_append_path.clone() {
                    // ...existing code...
                    let _ = std::fs::OpenOptions::new().create(true).append(true).open(path);
                }
                                if exe_path.exists() && exe_path.is_file() {
                                    println!("{} is {}", phrase, exe_path.display());
                                    found = true;
                                    break;
                                }
                            }
                            if !found {
                                println!("{}: not found", phrase);
                            }
                        }
                    }
                    "pwd" => {
                        let current_dir = env::current_dir().unwrap();
                        println!("{}", current_dir.display());
                    }
                    "cd" => {
                        let new_dir = args.get(0).unwrap_or(&"");
                        if new_dir.is_empty() {
                            let home_dir = env::var("HOME").unwrap_or_default();
                            if let Err(_) = env::set_current_dir(&home_dir) {
                                eprintln!("cd: {}: Unable to change directory", home_dir);
                            }
                        } else {
                            let expanded = if new_dir.starts_with('~') {
                                let home_dir = env::var("HOME").unwrap_or_default();
                                new_dir.replacen('~', &home_dir, 1)
                            } else {
                                new_dir.to_string()
                            };
                            let path = Path::new(&expanded);
                            if let Err(_) = env::set_current_dir(path) {
                                eprintln!("cd: {}: No such file or directory", expanded);
                            }
                        }
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
                                    if stderr_redirect_path.is_some() {
                                        err_output.push_str(&msg);
                                    } else if stderr_append_path.is_some() {
                                        err_output.push_str(&msg);
                                    } else {
                                        eprint!("{}", msg);
                                    }
                                }
                            }
                        }
                        if let Some(path) = redirect_path {
                            let _ = std::fs::write(path, output);
                        } else if let Some(path) = append_redirect_path {
                            use std::io::Write as _;
                            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                                let _ = write!(file, "{}", output);
                            }
                        } else if !output.is_empty() {
                            if output.ends_with('\n') {
                                print!("{}", output);
                            } else {
                                println!("{}", output);
                            }
                        }
                        if let Some(path) = stderr_redirect_path {
                            let _ = std::fs::write(path, err_output);
                        } else if let Some(path) = stderr_append_path {
                            use std::io::Write as _;
                            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                                let _ = write!(file, "{}", err_output);
                            }
                        }
                    }
                    _ => {
                        if let Some(exe_path) = find_executable_in_path(command) {
                            let output = Command::new(&exe_path)
                                .args(&args)
                                .output()
                                .expect("failed to execute process");

                            let stdout_str = String::from_utf8_lossy(&output.stdout);
                            let stderr_str = String::from_utf8_lossy(&output.stderr);

                            let path_prefix = exe_path.parent().unwrap().to_str().unwrap();
                            let modified_stdout = stdout_str.replace(&format!("{}/", path_prefix), "");
                            let modified_stderr = stderr_str.replace(&format!("{}/", path_prefix), "");

                            if let Some(path) = redirect_path {
                                let _ = std::fs::write(path, modified_stdout);
                            } else if let Some(path) = append_redirect_path {
                                use std::io::Write as _;
                                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                                    let _ = write!(file, "{}", modified_stdout);
                                }
                            } else {
                                io::stdout().write_all(modified_stdout.as_bytes()).unwrap();
                            }
                            if let Some(path) = stderr_redirect_path {
                                let _ = std::fs::write(path, modified_stderr);
                            } else if let Some(path) = stderr_append_path {
                                use std::io::Write as _;
                                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                                    let _ = write!(file, "{}", modified_stderr);
                                }
                            } else {
                                io::stderr().write_all(modified_stderr.as_bytes()).unwrap();
                            }
                        } else {
                            println!("{}: command not found", command);
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}
