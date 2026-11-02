//! Advisory static context only: no error text or runtime authority crosses the output boundary.
use super::*;
use std::fmt;

macro_rules! codes {
    ($($name:ident => ($stage:literal, $reason:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(super) enum Code { $($name),+ }
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
    if let Err(error) = &result {
        use std::io::Write;
        // A broken diagnostic sink cannot replace the original admission failure.
        let _ = writeln!(std::io::stderr().lock(), "{}", code(error).line());
    }
    result
}
