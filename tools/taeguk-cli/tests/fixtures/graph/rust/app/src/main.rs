mod cli;
mod globbed;
mod ops;
mod scoped;

fn main() {
    cli::run();
    let _ = ops::go();
    scoped::one();
    scoped::two();
    globbed::f();
}
