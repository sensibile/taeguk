#[derive(Debug, Clone, PartialEq)]
pub struct Plain;

pub fn plain(x: Option<fx_core::model::Item>) -> bool {
    let v = vec![crate::cli::widgets::draw as usize; 2];
    matches!(x, Some(fx_core::model::Item)) && !v.is_empty()
}
