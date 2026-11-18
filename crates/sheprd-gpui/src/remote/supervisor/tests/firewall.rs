//! The firewall check after a service change: run once the service runs,
//! kept until the next change, and skipped while off or after a failure.

use super::*;

static CHECKS: AtomicUsize = AtomicUsize::new(0);

#[gpui::test]
fn a_service_change_checks_the_firewall_approval(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Supervisor::fixture(cx, on(), silent_daemon);
        let supervisor = cx.global_mut::<Supervisor>();
        supervisor.reconcile = |_, _| Ok(Outcome::Installed);
        supervisor.check_approval = || {
            CHECKS.fetch_add(1, Ordering::SeqCst);
            Approval::ThisBuild
        };
        assert!(!Supervisor::approval_per_build(cx));
        Supervisor::apply_config(cx, RemoteConfig { port: 9000, ..on() });
    });
    cx.run_until_parked();
    assert_eq!(CHECKS.load(Ordering::SeqCst), 1);
    cx.update(|cx| assert!(Supervisor::approval_per_build(cx)));
    // Turning it off drops the warning without asking the firewall.
    cx.update(|cx| Supervisor::apply_config(cx, RemoteConfig::default()));
    cx.run_until_parked();
    assert_eq!(CHECKS.load(Ordering::SeqCst), 1);
    cx.update(|cx| assert!(!Supervisor::approval_per_build(cx)));
}

#[gpui::test]
fn a_restart_checks_again_and_a_failure_clears_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Supervisor::fixture(cx, on(), silent_daemon);
        cx.global_mut::<Supervisor>().check_approval = || Approval::ThisBuild;
        assert!(Supervisor::restart(cx));
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(Supervisor::approval_per_build(cx));
        cx.global_mut::<Supervisor>().restart =
            |_| Err(crate::remote::Error::ProgramMissing.into());
        assert!(Supervisor::restart(cx));
    });
    cx.run_until_parked();
    cx.update(|cx| assert!(!Supervisor::approval_per_build(cx)));
}

#[gpui::test]
fn a_signed_daemon_raises_no_warning(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Supervisor::fixture(cx, on(), silent_daemon);
        cx.global_mut::<Supervisor>().check_approval = || Approval::Lasting;
        assert!(Supervisor::restart(cx));
    });
    cx.run_until_parked();
    cx.update(|cx| assert!(!Supervisor::approval_per_build(cx)));
}
