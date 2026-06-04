pub fn parse_input(input: &str) -> Vec<String> {
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
