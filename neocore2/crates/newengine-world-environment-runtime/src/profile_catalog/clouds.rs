use super::CloudProfileDescriptor;

pub(super) const CLOUD_PROFILES: [CloudProfileDescriptor; 5] = [
    CloudProfileDescriptor {
        id: "clouds/temperate_cumulus.ycloud@profile",
        low_coverage_scale: 0.66,
        low_overcast_coverage_gain: 0.22,
        low_density_scale: 0.44,
        high_cloud_coverage_scale: 0.20,
        high_density_scale: 0.34,
    },
    CloudProfileDescriptor {
        id: "clouds/highlands_fields.ycloud@profile",
        low_coverage_scale: 0.70,
        low_overcast_coverage_gain: 0.24,
        low_density_scale: 0.47,
        high_cloud_coverage_scale: 0.22,
        high_density_scale: 0.35,
    },
    CloudProfileDescriptor {
        id: "clouds/default_temperate.ycloud@profile",
        low_coverage_scale: 0.66,
        low_overcast_coverage_gain: 0.22,
        low_density_scale: 0.44,
        high_cloud_coverage_scale: 0.20,
        high_density_scale: 0.34,
    },
    CloudProfileDescriptor {
        id: "clouds/alpine_winter.ycloud@profile",
        low_coverage_scale: 0.74,
        low_overcast_coverage_gain: 0.25,
        low_density_scale: 0.49,
        high_cloud_coverage_scale: 0.24,
        high_density_scale: 0.37,
    },
    CloudProfileDescriptor {
        id: "clouds/desert_dust.ycloud@profile",
        low_coverage_scale: 0.54,
        low_overcast_coverage_gain: 0.16,
        low_density_scale: 0.34,
        high_cloud_coverage_scale: 0.17,
        high_density_scale: 0.28,
    },
];
