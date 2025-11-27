use pathsearch::find_executable_in_path;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::{exit, Command};

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
            '\\' if !in_single_quotes && !in_double_quotes => {
                // Backslash escaping outside quotes: consume next char literally
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
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();

        
        const BUILTIN_CMDS: [&str; 5] = ["echo", "type", "exit", "pwd", "cd"];
        let tokens = parse_input(&input);

        if tokens.is_empty() {
            continue;
        }

        let command = &tokens[0];
        let args: Vec<&str> = tokens[1..].iter().map(|s| s.as_str()).collect();
        match command.as_str() {
            "exit" => exit(0),
            "echo" => {
           
                println!("{}", args.join(" "));
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
                for file_path in args {
                    let clean_path = file_path.trim_end_matches(&['\n', '\r'][..]);
                    match std::fs::read_to_string(clean_path) {
                        Ok(content) => output.push_str(&content),
                        Err(_) => eprintln!("cat: {}: No such file or directory", clean_path),
                    }
                }
                print!("{}", output);
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

                    io::stdout().write_all(modified_stdout.as_bytes()).unwrap();
                    io::stderr().write_all(modified_stderr.as_bytes()).unwrap();
                } else {
                    println!("{}: command not found", command);
                }
            }
        }
    }
}
