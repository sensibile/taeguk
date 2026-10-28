pub fn one() {
    use crate::cli as x;
    x::widgets::draw();
}

pub fn two() {
    use fx_core as x;
    let _ = x::extra::item();
}
