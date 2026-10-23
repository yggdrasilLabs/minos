//! Host-measured usage (LOOP-03; 07-RESEARCH Pattern 3) and the runner error's blame split (D-16).

use std::error::Error;
use std::time::Duration;

use minos_core::guest::SetupStep;
use minos_core::usage::Usage;
use minos_runner::{ConfigCause, InfraCause, RUNNER_VCPUS, RunnerError, host_usage};
use minos_sandbox::{Machine, SpecError};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn usage_is_elapsed_milliseconds_times_the_machine() -> TestResult {
    let machine = Machine::new(RUNNER_VCPUS, 2048)?;
    let usage = host_usage(Duration::from_millis(1500), &machine);
    assert_eq!(
        usage,
        Usage::compute(1500 * u64::from(RUNNER_VCPUS), 1500 * 2048)
    );
    Ok(())
}

#[test]
fn usage_truncates_sub_millisecond_time() -> TestResult {
    let machine = Machine::new(1, 128)?;
    assert_eq!(
        host_usage(Duration::from_micros(2999), &machine),
        Usage::compute(2, 256)
    );
    assert_eq!(host_usage(Duration::ZERO, &machine), Usage::compute(0, 0));
    Ok(())
}

#[test]
fn usage_saturates_instead_of_wrapping() -> TestResult {
    let machine = Machine::new(RUNNER_VCPUS, 2048)?;
    assert_eq!(
        host_usage(Duration::MAX, &machine),
        Usage::compute(u64::MAX, u64::MAX)
    );
    let near = Duration::from_millis(u64::MAX / 2);
    assert_eq!(
        host_usage(near, &machine),
        Usage::compute(u64::MAX - 1, u64::MAX)
    );
    Ok(())
}

#[test]
fn runner_vcpus_builds_a_machine() {
    assert!(Machine::new(RUNNER_VCPUS, 2048).is_ok());
}

#[test]
fn infra_errors_are_retried_and_config_errors_are_not() {
    let infra = RunnerError::from(InfraCause::Launch);
    let config = RunnerError::from(ConfigCause::ScratchFull);
    assert!(infra.is_infra());
    assert!(!config.is_infra());
    assert_eq!(infra.what(), "the sandbox VM could not be launched");
    assert_eq!(
        config.what(),
        "the build or run filled [environment] scratch_mib; raise it"
    );
}

#[test]
fn error_text_is_the_fixed_cause_text_through_the_source_chain() {
    let error = RunnerError::Infra {
        cause: InfraCause::GuestSetup {
            step: SetupStep::Input,
        },
    };
    assert_eq!(error.to_string(), "an infrastructure failure, retried");
    let source = error.source().map(ToString::to_string);
    assert_eq!(
        source.as_deref(),
        Some("the guest could not read its input disk")
    );
}

#[test]
fn a_spec_error_keeps_its_kind_as_the_source() {
    let error = SpecError::DurationOverCeiling { ms: 7_300_000 };
    let cause = ConfigCause::Spec { error };
    assert_eq!(
        cause.source().map(ToString::to_string),
        Some(error.to_string())
    );
    assert_eq!(
        RunnerError::from(cause).what(),
        "the sandbox spec could not be built; lower the stage budgets"
    );
}
