#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::all)]
#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::missing_errors_doc)]

mod_use::mod_use![bot, command, config, ext, util];

fn main() {
    println!("Hello, world!");
}
