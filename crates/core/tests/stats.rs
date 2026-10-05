use poko_core::retention;

const NOW: i64 = 1_791_140_400;

#[test]
fn retention_is_point_nine_when_elapsed_equals_stability() {
    let stability = 10.0;
    let reviewed_at = NOW - (stability as i64) * 86_400;
    let got = retention(stability, reviewed_at, NOW);
    assert!((got - 0.9).abs() < 1e-4, "{got}");
}

#[test]
fn retention_is_one_when_no_time_has_elapsed() {
    let got = retention(4.0, NOW, NOW);
    assert!((got - 1.0).abs() < 1e-4, "{got}");
}

#[test]
fn negative_elapsed_is_clamped_to_zero() {
    let got = retention(10.0, NOW + 86_400, NOW);
    assert!((got - 1.0).abs() < 1e-4, "{got}");
}

#[test]
fn retention_matches_an_independent_fsrs6_calculation() {
    // Python で式だけを別計算した値。decay = 0.1542、
    // factor = 0.9 ** (1 / -decay) - 1、R = (30 / 10 * factor + 1) ** (-decay)。
    let reviewed_at = NOW - 30 * 86_400;
    let got = retention(10.0, reviewed_at, NOW);
    let expected = 0.809_388_103_573_170_8;
    assert!((got - expected).abs() < 1e-4, "{got}");
}
