//! Exercises the real-capture permission probe.

#[test]
fn probe_capture_matches_preflight_when_permission_is_granted() {
    // In environments where recording already works (CI agents with screen
    // capture allowed, or a dev machine that has granted permission), the
    // probe must agree with the preflight result. The probe is the fallback
    // used when preflight wrongly reports "not granted", so it has to be
    // correct in the positive case as well.
    let preflight = scap::has_permission();
    let probed = scap::probe_capture();
    assert_eq!(preflight, probed, "preflight={preflight} probed={probed}");
}
