pub fn f() {
    mod local {
        pub fn run() {
            crate::cli::run();
        }
    }
    local::run();
}

#[my_macros::wire]
pub fn g() {}

#[derive(serde::Serialize)]
pub struct S;

pub fn h() -> usize {
    vec![include!("inc_body.rs"); 1].len()
}
