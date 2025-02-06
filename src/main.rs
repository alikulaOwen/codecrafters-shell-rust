use std::env;
use std::io::{self, Write};
use std::process::{exit, Command};
use pathsearch::find_executable_in_path;

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();

        const BUILTIN_CMDS: [&str; 3] = ["echo", "type", "exit"];
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
            _ => {
                if let Some(exe) = find_executable_in_path(command) {
                    let output = Command::new(exe)
                        .args(&args) // Pass only the arguments
                        .output()
                        .expect("failed to execute process");

                    io::stdout().write_all(&output.stdout).unwrap();
                    io::stderr().write_all(&output.stderr).unwrap();
                } else {
                    println!("{}: command not found", command);
                }
            }
        }
    }
}