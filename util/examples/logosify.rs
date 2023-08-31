use std::io::{self, Read};

use util::Logos;

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    println!("{}", Logos::elite_new(&input));
}
