pub mod clock;
pub mod config;
pub mod model;
pub mod protocol;

use std::{error::Error, io};

use crate::protocol::{parse_event, read_init};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    read_init(&mut input)?;
    for line in io::BufRead::lines(input) {
        let line = line?;
        if let Err(error) = parse_event(&line) {
            eprintln!("{error}");
        }
    }
    Ok(())
}
