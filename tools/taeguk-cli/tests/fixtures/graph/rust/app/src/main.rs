mod cli;
mod declared;
mod facade_user;
mod globbed;
mod loop_;
mod macros;
mod many;
mod ops;
mod plat;
mod scoped;

fn main() {
    cli::run();
    let _ = ops::go();
    scoped::one();
    scoped::two();
    globbed::f();
    facade_user::g();
    plat::go();
    declared::f();
}
