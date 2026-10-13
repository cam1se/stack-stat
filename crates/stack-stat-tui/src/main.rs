use clap::Parser;

#[derive(Parser)]
#[command(
    name = "stack-stat",
    version,
    about = "Status for stacked pull requests"
)]
struct Cli {}

fn main() {
    Cli::parse();
}
