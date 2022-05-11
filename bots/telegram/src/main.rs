#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::all)]
#![allow(clippy::missing_errors_doc)]

mod_use::mod_use![bot, command, config, ext];

fn main() {
    println!("Hello, world!");
}
