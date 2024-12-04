use std::env;

const TARGET: &str = "thumbv7em-none-eabihf";

fn main() {
    if env::var("TARGET").unwrap() == TARGET {
        // link with libc nano so post boot code can use the printf and string functions
        println!("cargo:rustc-link-search={}", env::current_dir().expect("could not get current directory").display());
        println!("cargo:rustc-link-lib=c_nano");
    } else {
        println!("cargo:warning=Unsupported target! Please run with '--target thumbv7em-none-eabihf'.")
    }
}
