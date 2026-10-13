fn main() {
    let target = std::env::var("TARGET").expect("Cargo supplies a target triple");
    println!("cargo:rustc-env=STACK_STAT_TARGET={target}");
}
