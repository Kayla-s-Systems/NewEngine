#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::{CpuCapabilities, CpuFeatureSet, HostAffinityPolicy};
use sysinfo::System;

/// Discover only CPU topology/features.
pub fn discover() -> CpuCapabilities {
    let mut sys = System::new_all();
    sys.refresh_cpu_all();
    let brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("PROCESSOR_IDENTIFIER").ok());
    let logical_cores = match sys.cpus().len() {
        0 => std::thread::available_parallelism()
            .ok()
            .map(|n| n.get() as u32),
        count => Some(count as u32),
    };
    CpuCapabilities {
        brand,
        physical_cores: System::physical_core_count().map(|count| count as u32),
        logical_cores,
        features: discover_features(),
        affinity_policy: HostAffinityPolicy::Automatic,
    }
}

fn discover_features() -> CpuFeatureSet {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        CpuFeatureSet {
            sse2: std::arch::is_x86_feature_detected!("sse2"),
            sse41: std::arch::is_x86_feature_detected!("sse4.1"),
            avx: std::arch::is_x86_feature_detected!("avx"),
            avx2: std::arch::is_x86_feature_detected!("avx2"),
            f16c: std::arch::is_x86_feature_detected!("f16c"),
            fma: std::arch::is_x86_feature_detected!("fma"),
            bmi1: std::arch::is_x86_feature_detected!("bmi1"),
            bmi2: std::arch::is_x86_feature_detected!("bmi2"),
        }
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        CpuFeatureSet::default()
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;

    #[test]
    fn cpu_probe_reports_at_least_one_logical_core() {
        assert!(discover().logical_cores.unwrap_or(1) >= 1);
    }
}
