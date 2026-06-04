/*
 * Thought Process:
 * - This module handles pipeline chaining (e.g., 'cmd1 | cmd2 | cmd3').
 * - We iterate through command tokens, spawning child processes sequentially. We redirect the stdout of the
 *   previous command to a pipe, capture the output if the previous command was a shell builtin (running
 *   in-process), and write it to the stdin of the next child process using standard UNIX/OS pipe patterns.
 *
 * External Packages / Crates Alternative:
 * - 'duct' or 'subprocess' can be used to construct, pipe, and run processes easily.
 * - 'os_pipe' can be used for cross-platform piping of standard streams.
 *
 * Why External Packages are Easier:
 * - Managing OS pipes, deadlocks (caused by not closing pipe ends in the parent process), background children,
 *   and redirecting stderr/stdout correctly across Unix and Windows is extremely tricky. Libraries like 'duct'
 *   handle all process lifetime management, pipe redirection, and exit status aggregation with a simple,
 *   leak-safe builder API.
 */

use std::process::{Command, Stdio};
use std::io::Write;
use pathsearch::find_executable_in_path;
use crate::builtin::run_builtin;

pub fn execute_pipeline(commands: &[Vec<String>]) {
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
