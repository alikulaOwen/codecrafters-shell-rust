fn longest_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    let mut prefix = strings[0].clone();
    for s in strings.iter().skip(1) {
        let mut i = 0;
        let min_len = std::cmp::min(prefix.len(), s.len());
        while i < min_len && prefix.as_bytes()[i] == s.as_bytes()[i] {
            i += 1;
        }
        prefix.truncate(i);
        if prefix.is_empty() {
            break;
        }
    }
    prefix
}
use pathsearch::find_executable_in_path;
use rustyline::completion::{Completer, Pair};
use rustyline::config::{Config, CompletionType};
use rustyline::{Editor, Helper, Context};
use rustyline::hint::Hinter;
use rustyline::highlight::Highlighter;
use std::env;
use std::io::{self, Write};

use std::process::{exit, Command};

struct BuiltinCompleter;
use std::cell::RefCell;

thread_local! {
    static TAB_COUNT: RefCell<u8> = RefCell::new(0);
}

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
        let mut all_matches = Vec::new();
        
        // Builtins
        for &cmd in &candidates {
            if cmd.starts_with(fragment) {
                all_matches.push(cmd.to_string());
            }
        }
        
        // External executables in PATH
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in path_var.split(':') {
                let path = std::path::Path::new(dir);
                if !path.exists() { continue; }
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let file_path = entry.path();
                        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                            if file_name.starts_with(fragment) {
                                #[cfg(unix)]
                                {
                                    use std::os::unix::fs::PermissionsExt;
                                    if let Ok(meta) = entry.metadata() {
                                        if meta.permissions().mode() & 0o111 != 0 {
                                            all_matches.push(file_name.to_string());
                                        }
                                    }
                                }
                                #[cfg(not(unix))]
                                {
                                    all_matches.push(file_name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Remove duplicates and sort
        let mut seen = std::collections::HashSet::new();
        all_matches.retain(|x| seen.insert(x.clone()));
        all_matches.sort();

        if all_matches.is_empty() {
            return Ok((0, vec![]));
        }

        // Find the longest common prefix
        let common_prefix = longest_common_prefix(&all_matches);
        
        // If the common prefix is the same as the fragment, there's nothing to complete
        // But we still return all matches so they can be displayed on double-TAB
        if common_prefix == fragment {
            let result: Vec<Pair> = all_matches.iter().map(|m| Pair {
                display: m.clone(),
                replacement: m.clone(),
            }).collect();
            return Ok((0, result));
        }
        
        // There's a common prefix beyond the fragment - complete to it
        // Add trailing space only if there's exactly one match
        let completion = if all_matches.len() == 1 {
            format!("{} ", common_prefix)
        } else {
            common_prefix.clone()
        };
        
        let result = vec![Pair {
            display: completion.clone(),
            replacement: completion,
        }];

        Ok((0, result))
    }
    }

impl Hinter for BuiltinCompleter {
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for BuiltinCompleter {}

impl Helper for BuiltinCompleter {}

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

fn run_builtin(
    command: &str,
    args: &[&str],
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
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

fn execute_pipeline(commands: &[Vec<String>]) {
    use std::process::Stdio;
    
    if commands.is_empty() {
        eprintln!("Invalid pipeline: no commands");
        return;
    }
    
    for cmd_tokens in commands {
        if cmd_tokens.is_empty() {
            eprintln!("Invalid pipeline: empty command");
            return;
        }
    }
    
    // We'll chain commands together, maintaining the previous command's output
    let mut previous_output: Option<Vec<u8>> = None;
    let mut previous_child: Option<std::process::Child> = None;
    
    for (i, cmd_tokens) in commands.iter().enumerate() {
        let cmd = &cmd_tokens[0];
        let args: Vec<&str> = cmd_tokens[1..].iter().map(|s| s.as_str()).collect();
        let is_last = i == commands.len() - 1;
        
        // Check if this is a builtin command
        let is_builtin = run_builtin(cmd, &args, &mut Vec::new(), &mut Vec::new(), None);
        
        if is_builtin {
            // Handle builtin command
            let mut capture = Vec::new();
            
            // Wait for previous external command if any
            if let Some(mut child) = previous_child.take() {
                let _ = child.wait();
            }
            
            // Run the builtin
            if is_last {
                // Last command: output to real stdout
                run_builtin(cmd, &args, &mut std::io::stdout(), &mut std::io::stderr(), None);
            } else {
                // Not the last command: capture output for next command
                run_builtin(cmd, &args, &mut capture, &mut std::io::stderr(), None);
                previous_output = Some(capture);
            }
            previous_child = None;
            
        } else {
            // Handle external command
            if let Some(exe) = find_executable_in_path(cmd) {
                let mut command = Command::new(&exe);
                command.args(&args);
                
                // Set up stdin
                if let Some(output) = previous_output.take() {
                    // Previous command was a builtin with captured output
                    command.stdin(Stdio::piped());
                    if !is_last {
                        command.stdout(Stdio::piped());
                    }
                    
                    match command.spawn() {
                        Ok(mut child) => {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(&output);
                            }
                            
                            if is_last {
                                let _ = child.wait();
                            } else {
                                previous_child = Some(child);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to execute {}: {}", cmd, e);
                            return;
                        }
                    }
                } else if let Some(mut child) = previous_child.take() {
                    // Previous command was an external command
                    if let Some(stdout) = child.stdout.take() {
                        command.stdin(stdout);
                    }
                    if !is_last {
                        command.stdout(Stdio::piped());
                    }
                    
                    match command.spawn() {
                        Ok(mut child2) => {
                            let _ = child.wait();
                            if is_last {
                                let _ = child2.wait();
                            } else {
                                previous_child = Some(child2);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to execute {}: {}", cmd, e);
                            let _ = child.kill();
                            return;
                        }
                    }
                } else {
                    // First command in the pipeline
                    if !is_last {
                        command.stdout(Stdio::piped());
                    }
                    
                    match command.spawn() {
                        Ok(mut child) => {
                            if is_last {
                                let _ = child.wait();
                            } else {
                                previous_child = Some(child);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to execute {}: {}", cmd, e);
                            return;
                        }
                    }
                }
            } else {
                eprintln!("{}: command not found", cmd);
                // Clean up any running child process
                if let Some(mut child) = previous_child.take() {
                    let _ = child.kill();
                }
                return;
            }
        }
    }
    
    // Wait for any remaining child process
    if let Some(mut child) = previous_child {
        let _ = child.wait();
    }
}

fn main() {
    // Define default history file path
    let history_file = env::var("HOME")
        .map(|home| format!("{}/.shell_history", home))
        .unwrap_or_else(|_| ".shell_history".to_string());
    
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .max_history_size(1000)
        .build();
    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(BuiltinCompleter));
    
    // Load history from file on startup (ignore errors if file doesn't exist)
    let _ = rl.load_history(&history_file);
    
    loop {
        let input = rl.readline("$ ");
        match input {
            Ok(line) => {
                // Add to history
                let _ = rl.add_history_entry(&line);
                
                let tokens = parse_input(&line);

                if tokens.is_empty() {
                    continue;
                }

                // Special handling for history -r <path>
                if tokens.len() == 3 && tokens[0] == "history" && tokens[1] == "-r" {
                    let history_file = &tokens[2];
                    if let Err(e) = rl.load_history(history_file) {
                        eprintln!("history: {}: {}", history_file, e);
                    }
                    continue;
                }

                // Special handling for history -w <path>
                if tokens.len() == 3 && tokens[0] == "history" && tokens[1] == "-w" {
                    let history_file = &tokens[2];
                    if let Err(e) = rl.save_history(history_file) {
                        eprintln!("history: {}: {}", history_file, e);
                    }
                    continue;
                }

                // Special handling for history -a <path>
                if tokens.len() == 3 && tokens[0] == "history" && tokens[1] == "-a" {
                    let history_file_path = &tokens[2];
                    
                    // Get current history
                    let current_history: Vec<String> = rl.history().iter().map(|s| s.to_string()).collect();
                    
                    // Append all current history entries to file
                    match std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(history_file_path)
                    {
                        Ok(mut file) => {
                            use std::io::Write;
                            for entry in current_history {
                                if let Err(e) = writeln!(file, "{}", entry) {
                                    eprintln!("history: {}: {}", history_file_path, e);
                                    break;
                                }
                            }
                        }
                        Err(e) => eprintln!("history: {}: {}", history_file_path, e),
                    }
                    continue;
                }

                // Check for pipeline operator
                if tokens.contains(&"|".to_string()) {
                    // Split tokens by pipe operator to get all commands in the pipeline
                    let mut pipeline_commands: Vec<Vec<String>> = Vec::new();
                    let mut current_cmd: Vec<String> = Vec::new();
                    
                    for token in &tokens {
                        if token == "|" {
                            if current_cmd.is_empty() {
                                eprintln!("Invalid pipeline syntax");
                                continue;
                            }
                            pipeline_commands.push(current_cmd.clone());
                            current_cmd.clear();
                        } else {
                            current_cmd.push(token.clone());
                        }
                    }
                    
                    // Add the last command
                    if current_cmd.is_empty() {
                        eprintln!("Invalid pipeline syntax");
                        continue;
                    }
                    pipeline_commands.push(current_cmd);
                    
                    // Execute the pipeline
                    execute_pipeline(&pipeline_commands);
                    continue;
                }

                let mut tokens = tokens; // Make mutable for redirection handling

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

                // Prepare output writers based on redirection
                let mut stdout: Box<dyn Write> = if let Some(path) = &redirect_path {
                    match std::fs::File::create(path) {
                        Ok(f) => Box::new(f),
                        Err(e) => {
                            eprintln!("Failed to create file: {}", e);
                            continue;
                        }
                    }
                } else if let Some(path) = &append_redirect_path {
                    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
                        Ok(f) => Box::new(f),
                        Err(e) => {
                            eprintln!("Failed to open file: {}", e);
                            continue;
                        }
                    }
                } else {
                    Box::new(io::stdout())
                };

                let mut stderr: Box<dyn Write> = if let Some(path) = &stderr_redirect_path {
                    match std::fs::File::create(path) {
                        Ok(f) => Box::new(f),
                        Err(e) => {
                            eprintln!("Failed to create file: {}", e);
                            continue;
                        }
                    }
                } else if let Some(path) = &stderr_append_path {
                    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
                        Ok(f) => Box::new(f),
                        Err(e) => {
                            eprintln!("Failed to open file: {}", e);
                            continue;
                        }
                    }
                } else {
                    Box::new(io::stderr())
                };

                let command = &tokens[0];
                let args: Vec<&str> = tokens[1..].iter().map(|s| s.as_str()).collect();

                // Get history for builtin commands
                let history_vec: Vec<String> = rl.history().iter().map(|s| s.to_string()).collect();
                
                // Try running as builtin
                if !run_builtin(command, &args, &mut stdout, &mut stderr, Some(&history_vec)) {
                    // Not a builtin, try external
                    if let Some(_exe_path) = find_executable_in_path(command) {
                        // For external commands, we need to handle redirection differently
                        // because Command::new uses Stdio, not Write trait objects.
                        // But we already created Write objects.
                        // We can't easily convert Write back to Stdio/File.
                        // So we should probably check builtin FIRST, then do redirection setup for external differently?
                        // OR, we can just use the paths again.
                        
                        // Re-evaluating logic:
                        // The previous code handled redirection inside each match arm or passed it.
                        // To keep it clean, let's revert the Write object creation for external commands
                        // and handle them using the paths variables which are still available.
                        
                        let stdout_stdio = if let Some(path) = &redirect_path {
                            match std::fs::File::create(path) {
                                Ok(f) => std::process::Stdio::from(f),
                                Err(_) => std::process::Stdio::null(),
                            }
                        } else if let Some(path) = &append_redirect_path {
                            match std::fs::OpenOptions::new().create(true).append(true).open(path) {
                                Ok(f) => std::process::Stdio::from(f),
                                Err(_) => std::process::Stdio::null(),
                            }
                        } else {
                            std::process::Stdio::inherit()
                        };

                        let stderr_stdio = if let Some(path) = &stderr_redirect_path {
                            match std::fs::File::create(path) {
                                Ok(f) => std::process::Stdio::from(f),
                                Err(_) => std::process::Stdio::null(),
                            }
                        } else if let Some(path) = &stderr_append_path {
                            match std::fs::OpenOptions::new().create(true).append(true).open(path) {
                                Ok(f) => std::process::Stdio::from(f),
                                Err(_) => std::process::Stdio::null(),
                            }
                        } else {
                            std::process::Stdio::inherit()
                        };

                        let child = Command::new(command)
                            .args(&args)
                            .stdout(stdout_stdio)
                            .stderr(stderr_stdio)
                            .spawn();
                            
                        match child {
                            Ok(mut c) => { let _ = c.wait(); },
                            Err(e) => eprintln!("Failed to execute process: {}", e),
                        }
                    } else {
                        println!("{}: command not found", command);
                    }
                }
            }
            Err(_) => break,
        }
    }
    
    // Save history on exit
    let _ = rl.save_history(&history_file);
}
