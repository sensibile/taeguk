use crate::cli as ui;
use fx_core as fc;

pub fn go() -> fc::model::Item {
    ui::widgets::draw();
    fc::model::Item
}
