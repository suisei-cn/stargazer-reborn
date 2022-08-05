use core_derive::Config;

#[derive(Config)]
struct Test {
    #[config(default_str = "10", inherit)]
    a: String,
}

fn main() {}
