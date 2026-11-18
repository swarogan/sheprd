use super::*;

#[test]
fn double_click_selects_the_whole_field() {
    let mut input = DialogInput::new("my space".into());
    input.click(3, 1, false);
    assert_eq!(input.selection, 3..3);
    input.click(3, 2, false);
    assert_eq!(input.selection, 0..8);
    assert!(!input.reversed);
    input.click(0, 3, false);
    assert_eq!(input.selection, 0..8);
}

#[test]
fn single_click_with_shift_extends_the_selection() {
    let mut input = DialogInput::new("my space".into());
    input.click(5, 1, false);
    input.click(2, 1, true);
    assert_eq!(input.selection, 2..5);
    assert!(input.reversed);
}
