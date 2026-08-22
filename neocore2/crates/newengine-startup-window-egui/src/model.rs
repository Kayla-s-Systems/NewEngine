#![forbid(unsafe_op_in_unsafe_fn)]

use super::StartupWindowSelection;

#[derive(Clone, Debug)]
pub(super) enum PresenterOutcome {
    Pending,
    Confirmed(StartupWindowSelection),
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SettingsPage {
    Display,
    Quality,
    Effects,
    Advanced,
}

impl SettingsPage {
    pub(super) const ALL: [Self; 4] = [Self::Display, Self::Quality, Self::Effects, Self::Advanced];

    pub(super) const fn number(self) -> &'static str {
        match self {
            Self::Display => "01",
            Self::Quality => "02",
            Self::Effects => "03",
            Self::Advanced => "04",
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::Quality => "Quality",
            Self::Effects => "Effects",
            Self::Advanced => "Advanced",
        }
    }

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Display => "Display & Presentation",
            Self::Quality => "Graphics Quality",
            Self::Effects => "Anti-Aliasing & Post-FX",
            Self::Advanced => "Core Launch Contract",
        }
    }

    pub(super) const fn description(self) -> &'static str {
        match self {
            Self::Display => {
                "Configure the native output surface before platform and renderer creation."
            }
            Self::Quality => "Select resource quality, shadows and a reusable graphics baseline.",
            Self::Effects => "Tune independent renderer variables and post-processing parameters.",
            Self::Advanced => "Inspect the resolved core snapshot and exported process variables.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StatusKind {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RenderPressure {
    Low,
    Balanced,
    High,
    Extreme,
}

impl RenderPressure {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Balanced => "BALANCED",
            Self::High => "HIGH",
            Self::Extreme => "EXTREME",
        }
    }

    pub(super) const fn detail(self) -> &'static str {
        match self {
            Self::Low => "Conservative GPU workload",
            Self::Balanced => "Recommended real-time baseline",
            Self::High => "Heavy effects or supersampling",
            Self::Extreme => "Potentially expensive combined stack",
        }
    }
}
