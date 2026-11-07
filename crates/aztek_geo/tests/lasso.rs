//! Tests: lasso.

use aztek_geo::lasso;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn contains_the_points_inside_its_outline() {
    let square = [v(0.0, 0.0), v(10.0, 0.0), v(10.0, 10.0), v(0.0, 10.0)];
    assert!(lasso::contains(&square, v(5.0, 5.0)));
    assert!(lasso::contains(&square, v(0.5, 9.5)));
    assert!(!lasso::contains(&square, v(-1.0, 5.0)));
    assert!(!lasso::contains(&square, v(5.0, 11.0)));
    assert!(!lasso::contains(&square, v(20.0, 20.0)));

    // A U shape: the notch between the arms is outside.
    let u = [v(0.0, 0.0), v(10.0, 0.0), v(10.0, 10.0), v(7.0, 10.0), v(7.0, 3.0), v(3.0, 3.0), v(3.0, 10.0), v(0.0, 10.0)];
    assert!(lasso::contains(&u, v(1.5, 8.0)));
    assert!(lasso::contains(&u, v(8.5, 8.0)));
    assert!(lasso::contains(&u, v(5.0, 1.5)));
    assert!(!lasso::contains(&u, v(5.0, 6.0)));
}

#[test]
fn closed_implicitly_and_needs_three_points() {
    assert!(!lasso::contains(&[], v(0.0, 0.0)));
    assert!(!lasso::contains(&[v(0.0, 0.0), v(10.0, 10.0)], v(5.0, 5.0)));
    let tri = [v(0.0, 0.0), v(10.0, 0.0), v(0.0, 10.0)];
    assert!(lasso::contains(&tri, v(2.0, 2.0)));
    assert!(!lasso::contains(&tri, v(8.0, 8.0)));
}

#[test]
fn self_crossing_uses_even_odd_rule() {
    // A bow tie: the two lobes are inside, the area beyond the crossing is not.
    let bow = [v(0.0, 0.0), v(10.0, 10.0), v(10.0, 0.0), v(0.0, 10.0)];
    assert!(lasso::contains(&bow, v(2.0, 5.0)));
    assert!(lasso::contains(&bow, v(8.0, 5.0)));
    assert!(!lasso::contains(&bow, v(5.0, 1.0)));
}

#[test]
fn extend_skips_points_too_close_to_the_last() {
    let mut poly = Vec::new();
    assert!(lasso::extend(&mut poly, v(0.0, 0.0), 2.0));
    assert!(!lasso::extend(&mut poly, v(1.0, 1.0), 2.0));
    assert!(lasso::extend(&mut poly, v(3.0, 0.0), 2.0));
    assert_eq!(poly.len(), 2);
    let (lo, hi) = lasso::bounds(&[v(3.0, -1.0), v(-2.0, 4.0), v(1.0, 1.0)]);
    assert_eq!(lo, v(-2.0, -1.0));
    assert_eq!(hi, v(3.0, 4.0));
}
