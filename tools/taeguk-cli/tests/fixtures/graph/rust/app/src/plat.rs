#[cfg(unix)]
use crate::cli as imp;
#[cfg(windows)]
use fx_core::implementation as imp;

pub fn go() {
    imp::widgets::draw();
    imp::sub::f();
}
