use std::io::{self, Read};

use util::Logos;

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let logos: Logos = input
        .chars()
        .map(|c| match c.to_ascii_uppercase() {
            'E' => '3',
            'I' => '1',
            'O' => '0',
            'S' => '5',
            a => a,
        })
        .collect();

    println!("{logos}");
}
