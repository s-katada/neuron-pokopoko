use poko_core::study_day_start;

#[test]
fn before_rollover_belongs_to_previous_study_day() {
    // 2026-10-05 03:59:59 JST → 2026-10-04 04:00:00 JST
    assert_eq!(study_day_start(1_791_140_399), 1_791_054_000);
}

#[test]
fn rollover_instant_starts_the_new_study_day() {
    // 2026-10-05 04:00:00 JST → 2026-10-05 04:00:00 JST
    assert_eq!(study_day_start(1_791_140_400), 1_791_140_400);
}

#[test]
fn late_evening_stays_on_the_same_study_day() {
    // 2026-10-05 23:59:59 JST → 2026-10-05 04:00:00 JST
    assert_eq!(study_day_start(1_791_212_399), 1_791_140_400);
}

#[test]
fn month_boundary_uses_previous_study_day() {
    // 2026-11-01 03:00:00 JST → 2026-10-31 04:00:00 JST
    assert_eq!(study_day_start(1_793_469_600), 1_793_386_800);
}

#[test]
fn year_boundary_uses_previous_study_day() {
    // 2027-01-01 00:30:00 JST → 2026-12-31 04:00:00 JST
    assert_eq!(study_day_start(1_798_731_000), 1_798_657_200);
}
