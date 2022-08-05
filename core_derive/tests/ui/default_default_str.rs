use core_derive::Config;

#[derive(Config)]
struct Test {
    #[config(default = "10", default_str = "10")]
    a: usize,
}

fn main() {}
