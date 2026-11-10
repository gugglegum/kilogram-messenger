//! Advisory static context only: no error text or runtime authority crosses the output boundary.
use super::*;
use std::fmt;

macro_rules! codes {
    ($($name:ident => ($stage:literal, $reason:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(in super::super) enum Code { $($name),+ }
        impl Code {
            pub(super) fn line(self) -> &'static str {
                match self { $(Self::$name => concat!("S54_ADMISSION_DIAGNOSTIC v1 stage=", $stage, " reason=", $reason)),+ }
            }
            #[cfg(test)]
            pub(super) const ALL: &'static [Self] = &[$(Self::$name),+];
        }
    }
}
codes! {
    AdmissionUnknown => ("admission", "unknown"),
    RequiredInput => ("inputs", "required_input"),
    InputDigest => ("inputs", "digest"),
    InputPin => ("inputs", "pin_read"),
    InputDocument => ("inputs", "document"),
    PolicyUnknown => ("policy", "unknown"),
    PolicyBinding => ("policy", "binding"),
    Namespace => ("namespace", "binding"),
    Directory => ("directory", "binding"),
    SourceUnknown => ("source", "unknown"),
    SourceHead => ("source", "head"),
    SourceSet => ("source", "set"),
    SourcePin => ("source", "pin_read"),
    ImagesUnknown => ("images", "unknown"),
    ImageRoles => ("images", "roles"),
    ImagePin => ("images", "pin_read"),
    RuntimeUnknown => ("runtime", "unknown"),
    RuntimePolicy => ("runtime", "policy"),
    Environment => ("runtime", "environment"),
    RuntimeConfig => ("runtime", "config"),
    Selection => ("selection", "binding"),
    Activation => ("activation", "binding"),
    ActivationValidity => ("activation", "validity"),
    Launch => ("launch", "binding"),
    Capture => ("capture", "binding"),
    CompilerUnknown => ("compiler", "unknown"),
    NativeClaim => ("native_claim", "binding"),
    NativeCreate => ("native_claim", "create"),
    Clock => ("clock", "binding"),
    FReleaseTailBudget => ("f_ack_release", "tail_budget"),
    FReleasePauseBudget => ("f_ack_release", "pause_budget"),
    FReleaseControllerDeadline => ("f_ack_release", "controller_deadline"),
    FReleasePhaseDeadline => ("f_ack_release", "phase_deadline"),
    FReleaseOperationFailed => ("f_ack_release", "operation_failed"),
}

#[derive(Debug)]
struct ContextCode(Code);
impl fmt::Display for ContextCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.line())
    }
}
impl std::error::Error for ContextCode {}

/// The closure runs once, in place. Original errors remain in the anyhow chain.
pub(super) fn at<T>(code: Code, check: impl FnOnce() -> Result<T>) -> Result<T> {
    check().map_err(|error| {
        if error.downcast_ref::<ContextCode>().is_some() {
            error
        } else {
            error.context(ContextCode(code))
        }
    })
}
pub(super) fn code(error: &anyhow::Error) -> Code {
    // at() retains the first (most specific) typed check while errors unwind.
    error
        .downcast_ref::<ContextCode>()
        .map_or(Code::AdmissionUnknown, |c| c.0)
}
pub(super) fn report<T>(result: Result<T>) -> Result<T> {
    let selected = result.as_ref().err().map_or(Code::AdmissionUnknown, code);
    report_code(selected, result)
}
pub(super) fn report_code_to<T>(
    selected: Code,
    result: Result<T>,
    sink: &mut impl std::io::Write,
) -> Result<T> {
    if result.is_err() {
        // A broken sink cannot replace the original error chain.
        let _ = writeln!(sink, "{}", selected.line());
    }
    result
}
pub(in super::super) fn report_code<T>(selected: Code, result: Result<T>) -> Result<T> {
    report_code_to(selected, result, &mut std::io::stderr().lock())
}
