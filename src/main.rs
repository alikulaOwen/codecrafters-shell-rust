use pathsearch::find_executable_in_path;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::{exit, Command};

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();

        const BUILTIN_CMDS: [&str; 5] = ["echo", "type", "exit", "pwd", "cd"];
        let mut parts = input.trim().split_whitespace();
        let command = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();

        match command {
            "exit" => exit(0),
            "echo" => println!("{}", args.join(" ")),
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
                    env::set_current_dir(home_dir).unwrap();
                } else {
                    let new_path = Path::new(new_dir);
                    let absolute_path = if new_path.is_relative() {
                        let current_dir = env::current_dir().unwrap();
                        current_dir
                            .join(new_path)
                            .canonicalize()
                            .unwrap_or(current_dir.join(new_path))
                    } else {
                        new_path.to_path_buf()
                    };

                    if let Err(_e) = env::set_current_dir(absolute_path) {
                        eprintln!("cd: {}: No such file or directory", new_dir);
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

                    io::stdout().write_all(modified_stdout.as_bytes()).unwrap();
                    io::stderr().write_all(modified_stderr.as_bytes()).unwrap();
                } else {
                    println!("{}: command not found", command);
                }
            }
        }
    }
}
