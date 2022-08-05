use core_derive::Config;

#[derive(Config)]
struct Test {
    #[config(default = "10", inherit)]
    a: usize,
}

fn main() {}
