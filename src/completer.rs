use std::collections::HashMap;
use std::time::{Instant, Duration};
use rustyline::completion::{Completer, Pair};
use rustyline::{Helper, Context};
use rustyline::hint::Hinter;
use rustyline::highlight::Highlighter;

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

#[derive(Default)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    is_word: bool,
}

struct Trie {
    root: TrieNode,
}

impl Trie {
    fn new() -> Self {
        Trie {
            root: TrieNode::default(),
        }
    }

    fn insert(&mut self, word: &str) {
        let mut current = &mut self.root;
        for c in word.chars() {
            current = current.children.entry(c).or_default();
        }
        current.is_word = true;
    }

    fn get_words_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut current = &self.root;
        for c in prefix.chars() {
            if let Some(next) = current.children.get(&c) {
                current = next;
            } else {
                return Vec::new();
            }
        }
        let mut results = Vec::new();
        let mut current_word = prefix.to_string();
        self.collect_words(current, &mut current_word, &mut results);
        results
    }

    fn collect_words(&self, node: &TrieNode, current_word: &mut String, results: &mut Vec<String>) {
        if node.is_word {
            results.push(current_word.clone());
        }
        for (&c, child) in &node.children {
            current_word.push(c);
            self.collect_words(child, current_word, results);
            current_word.pop();
        }
    }
}

struct PathCache {
    trie: Trie,
    cached_path: String,
    last_updated: Instant,
}

impl PathCache {
    fn new() -> Self {
        PathCache {
            trie: Trie::new(),
            cached_path: String::new(),
            last_updated: Instant::now() - Duration::from_secs(3600),
        }
    }

    fn get_executables(&mut self, builtins: &[&str]) -> &Trie {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let now = Instant::now();
        
        if current_path != self.cached_path || now.duration_since(self.last_updated) > Duration::from_secs(30) {
            let mut new_trie = Trie::new();
            for &cmd in builtins {
                new_trie.insert(cmd);
            }
            
            #[cfg(unix)]
            let path_separator = ":";
            #[cfg(not(unix))]
            let path_separator = ";";

            for dir in current_path.split(path_separator) {
                let path = std::path::Path::new(dir);
                if !path.exists() {
                    continue;
                }
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let file_path = entry.path();
                        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(meta) = entry.metadata() {
                                    if meta.permissions().mode() & 0o111 != 0 && meta.is_file() {
                                        new_trie.insert(file_name);
                                    }
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                                    new_trie.insert(file_name);
                                }
                            }
                        }
                    }
                }
            }
            self.trie = new_trie;
            self.cached_path = current_path;
            self.last_updated = now;
        }
        &self.trie
    }
}

pub struct BuiltinCompleter {
    cache: std::cell::RefCell<PathCache>,
}

impl BuiltinCompleter {
    pub fn new() -> Self {
        BuiltinCompleter {
            cache: std::cell::RefCell::new(PathCache::new()),
        }
    }
}

fn get_completion_context(line: &str, pos: usize) -> (usize, Vec<String>, String, bool) {
    let mut words = Vec::new();
    let mut current_word = String::new();
    let mut word_start = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    let chars: Vec<(usize, char)> = line[..pos].char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (idx, c) = chars[i];
        if escaped {
            current_word.push(c);
            escaped = false;
            i += 1;
            continue;
        }

        match c {
            '\\' if !in_single_quote => {
                escaped = true;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                current_word.push(c);
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                current_word.push(c);
            }
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current_word.is_empty() {
                    words.push(current_word.clone());
                    current_word.clear();
                }
                word_start = idx + c.len_utf8();
            }
            _ => {
                current_word.push(c);
            }
        }
        i += 1;
    }

    let is_command_position = words.is_empty();
    (word_start, words, current_word, is_command_position)
}

fn complete_path(fragment: &str, only_dirs: bool) -> Vec<Pair> {
    let last_slash = fragment.rfind('/');
    let (dir_part, prefix_part) = match last_slash {
        Some(idx) => (&fragment[..=idx], &fragment[idx + 1..]),
        None => ("", fragment),
    };

    let expanded_dir = if dir_part.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_default();
        dir_part.replacen('~', &home, 1)
    } else if dir_part.is_empty() {
        "./".to_string()
    } else {
        dir_part.to_string()
    };

    let mut matches = Vec::new();
    let path = std::path::Path::new(&expanded_dir);
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            
            if name_str.starts_with(prefix_part) {
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if only_dirs && !is_dir {
                    continue;
                }
                
                let replacement = if is_dir {
                    format!("{}{}/", dir_part, name_str)
                } else {
                    format!("{}{}", dir_part, name_str)
                };
                
                let display = if is_dir {
                    format!("{}/", name_str)
                } else {
                    name_str.to_string()
                };

                matches.push((display, replacement, is_dir, name_str.starts_with('.')));
            }
        }
    }

    matches.sort_by(|a, b| {
        match (a.3, b.3) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => a.1.cmp(&b.1),
        }
    });

    matches.into_iter().map(|(display, replacement, _, _)| Pair {
        display,
        replacement,
    }).collect()
}

impl Completer for BuiltinCompleter {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), rustyline::error::ReadlineError> {
        let (word_start, words, fragment, is_command_position) = get_completion_context(line, pos);

        // Check if we should complete command names
        if is_command_position && !fragment.starts_with('.') && !fragment.starts_with('/') && !fragment.starts_with('~') {
            // Complete builtins and PATH executables
            let mut cache = self.cache.borrow_mut();
            let trie = cache.get_executables(&["echo", "type", "exit", "pwd", "cd", "history", "cat"]);
            let mut matches = trie.get_words_with_prefix(&fragment);
            matches.sort();

            if matches.is_empty() {
                return Ok((word_start, vec![]));
            }

            // Find the longest common prefix
            let common_prefix = longest_common_prefix(&matches);
            
            if matches.len() == 1 {
                let m = &matches[0];
                return Ok((word_start, vec![Pair {
                    display: m.clone(),
                    replacement: format!("{} ", m),
                }]));
            }

            if common_prefix == fragment {
                let result: Vec<Pair> = matches.into_iter().map(|m| Pair {
                    display: m.clone(),
                    replacement: m,
                }).collect();
                return Ok((word_start, result));
            }

            let result = vec![Pair {
                display: common_prefix.clone(),
                replacement: common_prefix,
            }];
            return Ok((word_start, result));
        }

        // Otherwise, complete path/file
        let only_dirs = !words.is_empty() && words[0] == "cd";
        let matches = complete_path(&fragment, only_dirs);

        if matches.is_empty() {
            return Ok((word_start, vec![]));
        }

        if matches.len() == 1 {
            let m = &matches[0];
            // If it's a directory (ends with '/'), don't add space. If it's a file, add space.
            let is_dir = m.replacement.ends_with('/');
            let replacement = if is_dir {
                m.replacement.clone()
            } else {
                format!("{} ", m.replacement)
            };
            return Ok((word_start, vec![Pair {
                display: m.display.clone(),
                replacement,
            }]));
        }

        // Longest common prefix of replacements
        let replacements: Vec<String> = matches.iter().map(|m| m.replacement.clone()).collect();
        let common_prefix = longest_common_prefix(&replacements);

        if common_prefix == fragment {
            return Ok((word_start, matches));
        }

        // If the common prefix is longer than the fragment, return that prefix as the single match
        let result = vec![Pair {
            display: common_prefix.clone(),
            replacement: common_prefix,
        }];
        Ok((word_start, result))
    }
}

impl Hinter for BuiltinCompleter {
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for BuiltinCompleter {}

impl Helper for BuiltinCompleter {}
