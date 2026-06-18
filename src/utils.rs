use std::io::{self, Read, Write};

pub fn catch_stdin() -> String {
    io::stdout().flush().expect("Failed to flush stdout");

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    input
}
