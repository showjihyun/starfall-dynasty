//! The first session's screens (the chapters themselves are the campaign QA's job).

#[test]
fn a_new_player_is_guided_from_the_landing_screen_to_chapter_one() {
    let r = bot::journey::run(&bot::journey::Options { campaign: false, ..Default::default() });
    assert_eq!(r.failed(), 0, "{}", r.text());
    assert!(r.to_first_ship <= 1.0, "own ship on screen within a second: {}", r.to_first_ship);
}
