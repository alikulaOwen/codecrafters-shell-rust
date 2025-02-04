#[allow(unused_imports)]
use std::io::{self, Write};
use std::process::exit;

fn main() {
    // Uncomment this block to pass the first stage
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        // Wait for user input
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        // treating all input as invalid command for now
        
        // if input.trim() == "exit 0" {
        //     break;
        // };
        match input.trim() {
            "exit 0" => exit(0),
            input if input.starts_with("echo ") => println!("{}", &input[5..]),
            input if input.starts_with("type") =>{
                let mut input = input.split_whitespace();
                input.next();
                let phrase = input.next().unwrap();
                match phrase {
                    "echo" => println!("echo is a shell builtin"),
                    "type" => println!("type is a shell builtin"),
                    _ => println!("{}: command not found", phrase),
                }
            }
            input => println!("{}: command not found", input),
        };


    }
}
