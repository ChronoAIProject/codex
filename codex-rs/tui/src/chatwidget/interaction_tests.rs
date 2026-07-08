use super::*;

#[test]
fn image_paste_shortcut_accepts_super_v() {
    assert!(is_image_paste_shortcut('v', KeyModifiers::SUPER));
}
