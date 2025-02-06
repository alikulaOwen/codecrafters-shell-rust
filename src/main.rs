use std::env;
use std::io::{self, Write};
use std::process::exit;
use pathsearch::find_executable_in_path;

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();

        const BUILTIN_CMDS: [&str; 3] = ["echo", "type", "exit"];
        match input.trim() {
            "exit 0" => exit(0),
            input if input.starts_with("echo ") => println!("{}", &input[5..]),
            input if input.starts_with("type") => {
                let mut input = input.split_whitespace();
                input.next();
                let phrase = input.next().unwrap();
                if BUILTIN_CMDS.contains(&phrase) {
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
            _ => println!("{}: command not found", input.trim()),
        }
    }
}