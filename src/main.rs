mod completer;
mod parser;
mod builtin;
mod pipeline;

use pathsearch::find_executable_in_path;
use rustyline::config::{Config, CompletionType};
use rustyline::Editor;
use std::env;
use std::io::{self, Write};
use std::process::{exit, Command};

/* 
 *  This file is part of the Rust-Shell project ()
 *  main.rs - The main entry point for the Rust-Shell application
 *  sets up the command-line interface, handles user input, and manages command execution
 *  uses rustyline for input handling and history management
 *  supports built-in commands, external commands, pipelines, and I/O redirection
 *  handles history file management with support for HISTFILE environment variable and default location
 *  implements special handling for the "history" command to allow loading and saving history from arbitrary files
 *  tracks the last history index appended to each file for efficient "history -a" implementation
 *  supports output redirection for both stdout and stderr with ">" and "2>" operators, as well as append modes with ">>" and "2>>"
 * handles pipelines by splitting input on "|" and executing each command in sequence, connecting stdout of one to stdin of the next
 * 
 */

fn main() {
    // Define history file path - check HISTFILE env var first, then fall back to default
    let history_file = env::var("HISTFILE").unwrap_or_else(|_| {
        env::var("HOME")
            .map(|home| format!("{}/.shell_history", home))
            .unwrap_or_else(|_| ".shell_history".to_string())
    });
    
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .max_history_size(1000)
        .build();
    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(completer::BuiltinCompleter::new()));
    
    // Load history from file on startup (ignore errors if file doesn't exist)
    let _ = rl.load_history(&history_file);
    
    // Track the last history index that was appended to each file
    let mut last_appended_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    
    loop {
        let input = rl.readline("$ ");
        match input {
            Ok(line) => {
                // Add to history
                let _ = rl.add_history_entry(&line);
                
                let tokens = parser::parse_input(&line);

                if tokens.is_empty() {
                    continue;
                }

                // Special handling for exit command - save history before exiting
                if tokens.len() == 1 && tokens[0] == "exit" {
                    let _ = rl.save_history(&history_file);
                    exit(0);
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
                    let history_file_path = tokens[2].clone();
                    
                    // Get current history
                    let current_history: Vec<String> = rl.history().iter().map(|s| s.to_string()).collect();
                    
                    // Get the starting index (where we left off last time)
                    let start_index = *last_appended_index.get(&history_file_path).unwrap_or(&0);
                    
                    // Only append new entries since last append
                    let new_entries: Vec<String> = current_history.iter().skip(start_index).cloned().collect();
                    
                    if !new_entries.is_empty() {
                        match std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&history_file_path)
                        {
                            Ok(mut file) => {
                                for entry in &new_entries {
                                    if let Err(e) = writeln!(file, "{}", entry) {
                                        eprintln!("history: {}: {}", history_file_path, e);
                                        break;
                                    }
                                }
                                // Update the last appended index for this file
                                last_appended_index.insert(history_file_path, current_history.len());
                            }
                            Err(e) => eprintln!("history: {}: {}", history_file_path, e),
                        }
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
                    pipeline::execute_pipeline(&pipeline_commands);
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
                if !builtin::run_builtin(command, &args, &mut stdout, &mut stderr, Some(&history_vec)) {
                    // Not a builtin, try external
                    if let Some(_exe_path) = find_executable_in_path(command) {
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
