mod cli;
mod ops;

fn main() {
    cli::run();
    let _ = ops::go();
}
