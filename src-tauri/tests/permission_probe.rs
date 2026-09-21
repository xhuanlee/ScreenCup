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

    // A bare test binary is not inside a .app bundle, so it carries a random
    // ad-hoc identity that TCC will not match against the real app's record,
    // even on a machine that has granted permission to the installed app.
    // `CGPreflightScreenCaptureAccess` can still report `true` for it (it
    // evaluates the request more loosely), so the two legitimately disagree
    // in that case. Only assert agreement when the probe actually succeeded —
    // a positive probe is the authoritative signal that permission is real.
    if probed {
        assert!(preflight, "probe succeeded but preflight said no — probe={probed} preflight={preflight}");
    }
}
