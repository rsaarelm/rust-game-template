use std::io::{self, Read};

use util::Silo;

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    println!("{}", Silo::new(&input));
}
