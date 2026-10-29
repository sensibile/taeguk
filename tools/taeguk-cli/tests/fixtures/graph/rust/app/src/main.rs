mod allow;
mod cfgderive;
mod cli;
mod declared;
mod derived;
mod ext;
mod facade_user;
mod globbed;
mod globmac;
mod hidden;
mod inc;
mod loop_;
mod macros;
mod many;
#[cfg_attr(unix, cfg_attr(unix, path = "cli.rs"))]
mod nested;
mod ops;
mod plat;
mod scoped;
mod stdalias;
mod used;

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
