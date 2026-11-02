//! Closed test purposes. A public document cannot invent a selector, root or budget.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Purpose {
    WholeP,
    FeasibilityF,
}
impl Purpose {
    pub(super) fn case(self) -> &'static str {
        match self {
            Self::WholeP => "P-s54-whole-resumed-l1-01",
            Self::FeasibilityF => "F-s54-mixed-hooks-feasibility-01",
        }
    }
    pub(super) fn attempt(self) -> String {
        format!("{}-attempt-01", self.case())
    }
    pub(super) fn selector(self) -> &'static str {
        match self {
            Self::WholeP => "live_l1::handoff::final_p5::s54_continuous_whole_resumed_l1_p5_public",
            Self::FeasibilityF => {
                "live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public"
            }
        }
    }
    pub(super) fn namespace(self) -> &'static str {
        match self {
            Self::WholeP => "migration-s54-integrated",
            Self::FeasibilityF => "migration-s54-feasibility",
        }
    }
    pub(super) fn public_namespace(self) -> &'static str {
        match self {
            Self::WholeP => "migration-s54-public-evidence",
            Self::FeasibilityF => "migration-s54-feasibility-public-evidence",
        }
    }
    pub(super) fn origin_name(self) -> &'static str {
        match self {
            Self::WholeP => "p5-origin",
            Self::FeasibilityF => "f-origin",
        }
    }
    pub(super) fn limits(self) -> [u64; 3] {
        match self {
            Self::WholeP => [2350, 2375, 2400],
            Self::FeasibilityF => [450, 480, 510],
        }
    }
    pub(super) fn inventory_class(self) -> &'static str {
        match self {
            Self::WholeP => "bound_s54_whole_inventory",
            Self::FeasibilityF => "bound_s54_feasibility_inventory",
        }
    }
    pub(super) fn activation_class(self) -> &'static str {
        match self {
            Self::WholeP => "root_s54_whole_activation",
            Self::FeasibilityF => "root_s54_feasibility_activation",
        }
    }
    pub(super) fn native_marker(self) -> &'static str {
        match self {
            Self::WholeP => "S54_FUNCTIONAL",
            Self::FeasibilityF => "S54_FEASIBILITY_FUNCTIONAL",
        }
    }
    pub(super) fn functional_class(self) -> &'static str {
        match self {
            Self::WholeP => "functional_assertions_completed",
            Self::FeasibilityF => "feasibility_assertions_completed",
        }
    }
    pub(super) fn run(self, root: &Path) -> PathBuf {
        root.join(".tmp")
            .join(self.namespace())
            .join(self.attempt())
    }
    pub(super) fn from_case(case: &str) -> Result<Self> {
        [Self::WholeP, Self::FeasibilityF]
            .into_iter()
            .find(|p| p.case() == case)
            .context("closed S54 purpose")
    }
    pub(super) fn identity(case: &str, attempt: &str, selector: &str) -> Result<Self> {
        let p = Self::from_case(case)?;
        ensure!(
            attempt == p.attempt() && selector == p.selector(),
            "S54 cross-purpose identity"
        );
        Ok(p)
    }
}
