use std::io::{self, Write};

pub fn catch_stdin() -> String {
    io::stdout().flush().expect("Failed to flush stdout");

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    input
}
