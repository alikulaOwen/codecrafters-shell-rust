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
use std::path::Path;
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
) -> bool {
    const BUILTIN_CMDS: [&str; 5] = ["echo", "type", "exit", "pwd", "cd"];

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
        _ => false,
    }
}

fn execute_pipeline(cmd1_tokens: &[String], cmd2_tokens: &[String]) {
    use std::process::Stdio;
    
    if cmd1_tokens.is_empty() || cmd2_tokens.is_empty() {
        eprintln!("Invalid pipeline: empty command");
        return;
    }

    let cmd1 = &cmd1_tokens[0];
    let args1: Vec<&str> = cmd1_tokens[1..].iter().map(|s| s.as_str()).collect();
    
    let cmd2 = &cmd2_tokens[0];
    let args2: Vec<&str> = cmd2_tokens[1..].iter().map(|s| s.as_str()).collect();

    // Handle first command (Builtin or External)
    let output1_opt: Option<Vec<u8>> = if run_builtin(cmd1, &args1, &mut Vec::new(), &mut Vec::new()) {
        // It's a builtin. Capture output.
        let mut capture = Vec::new();
        run_builtin(cmd1, &args1, &mut capture, &mut std::io::stderr());
        Some(capture)
    } else {
        None
    };

    let child1_opt = if output1_opt.is_none() {
        // External command
        if let Some(exe1) = find_executable_in_path(cmd1) {
            match Command::new(&exe1)
                .args(&args1)
                .stdout(Stdio::piped())
                .spawn()
            {
                Ok(child) => Some(child),
                Err(e) => {
                    eprintln!("Failed to execute {}: {}", cmd1, e);
                    return;
                }
            }
        } else {
            eprintln!("{}: command not found", cmd1);
            return;
        }
    } else {
        None
    };

    // Handle second command (Builtin or External)
    // It needs input from cmd1
    
    // If cmd2 is builtin
    if run_builtin(cmd2, &args2, &mut Vec::new(), &mut Vec::new()) {
        // Builtins in this shell don't really read stdin (except maybe cat if we implemented it, but we didn't implement stdin for cat)
        // So we just run it.
        // But we must consume/wait for cmd1.
        
        if let Some(mut child) = child1_opt {
            // External | Builtin
            // Wait for external. 
            // We should probably drop its stdout to avoid blocking if it writes a lot.
            // But for now, just waiting.
            let _ = child.wait();
        }
        
        // Run builtin to real stdout
        run_builtin(cmd2, &args2, &mut std::io::stdout(), &mut std::io::stderr());
        
    } else {
        // Cmd2 is External
        if let Some(exe2) = find_executable_in_path(cmd2) {
            let mut command = Command::new(&exe2);
            command.args(&args2);
            
            if let Some(input) = output1_opt {
                // Builtin | External
                command.stdin(Stdio::piped());
                match command.spawn() {
                    Ok(mut child) => {
                        if let Some(mut stdin) = child.stdin.take() {
                            let _ = stdin.write_all(&input);
                        }
                        let _ = child.wait();
                    }
                    Err(e) => eprintln!("Failed to execute {}: {}", cmd2, e),
                }
            } else if let Some(mut child1) = child1_opt {
                // External | External
                if let Some(stdout1) = child1.stdout.take() {
                    command.stdin(stdout1);
                    match command.spawn() {
                        Ok(mut child2) => {
                            let _ = child1.wait();
                            let _ = child2.wait();
                        }
                        Err(e) => {
                            eprintln!("Failed to execute {}: {}", cmd2, e);
                            let _ = child1.kill();
                        }
                    }
                }
            }
        } else {
            eprintln!("{}: command not found", cmd2);
        }
    }
}

fn main() {
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();
    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(BuiltinCompleter));
    loop {
        let input = rl.readline("$ ");
        match input {
            Ok(line) => {
                let tokens = parse_input(&line);

                if tokens.is_empty() {
                    continue;
                }

                // Check for pipeline operator
                if let Some(pipe_pos) = tokens.iter().position(|t| t == "|") {
                    if pipe_pos == 0 || pipe_pos == tokens.len() - 1 {
                        eprintln!("Invalid pipeline syntax");
                        continue;
                    }
                    let cmd1_tokens: Vec<String> = tokens[..pipe_pos].to_vec();
                    let cmd2_tokens: Vec<String> = tokens[pipe_pos + 1..].to_vec();
                    execute_pipeline(&cmd1_tokens, &cmd2_tokens);
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

                // Try running as builtin
                if !run_builtin(command, &args, &mut stdout, &mut stderr) {
                    // Not a builtin, try external
                    if let Some(exe_path) = find_executable_in_path(command) {
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

                        let mut child = Command::new(command)
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
}
