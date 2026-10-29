use crate::cli as x;
use x as y;

pub fn f() {
    use fx_core as x;
    y::widgets::draw();
    let _ = x::model::Item;
}
