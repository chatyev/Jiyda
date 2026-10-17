//! Модульные тесты src/frame.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn physical_rect_rounds_outward() {
    assert_eq!(to_physical((10.0, 10.0, 100.0, 20.0), 1.0), (10, 10, 110, 30));
    assert_eq!(to_physical((10.3, 0.5, 10.0, 10.0), 1.25), (12, 0, 26, 14));
    assert_eq!(to_physical((5.0, 5.0, -3.0, 0.0), 1.5), (7, 7, 7, 8));
}

#[test]
fn caption_buttons_sit_at_right_edge_without_gaps() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let b = caption_buttons(1000, scale);
        assert_eq!(b[2].2, 1000, "{scale}");
        assert_eq!(b[0].2, b[1].0);
        assert_eq!(b[1].2, b[2].0);
        assert!(b.iter().all(|r| r.1 == 0 && r.3 == (CHROME_H * scale).round() as i32));
    }
}

#[test]
fn hex_colors() {
    assert_eq!(parse_hex("#0b0F14"), Some((11, 15, 20)));
    for bad in ["", "0b0f14", "#0b0f1", "#0b0f1g", "#0b0f140"] {
        assert_eq!(parse_hex(bad), None, "{bad}");
    }
}
