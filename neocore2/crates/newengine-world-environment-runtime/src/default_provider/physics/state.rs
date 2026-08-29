use newengine_world_environment_api::{
    AtmosphericLayerDto, EnvironmentAtmosphereCellDto, EnvironmentFrameDto, PrecipitationKind,
    Vec3Dto,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ColumnMemory {
    pub world_time_seconds: f64,
    pub pressure_hpa: f32,
    pub temperature_c: f32,
    pub specific_humidity: f32,
    pub aerosol_mass: f32,
    pub cloud_water_path_kg_m2: f32,
    pub precipitation_rate_mm_h: f32,
}

impl ColumnMemory {
    #[inline]
    pub(crate) fn from_frame(frame: &EnvironmentFrameDto) -> Self {
        Self {
            world_time_seconds: frame.world_time_seconds,
            pressure_hpa: frame.atmosphere.surface_pressure_hpa,
            temperature_c: frame.atmosphere.temperature_celsius,
            specific_humidity: frame.atmosphere.specific_humidity_g_per_kg * 0.001,
            aerosol_mass: frame.atmosphere.aerosol_density,
            cloud_water_path_kg_m2: frame.atmosphere.cloud_water_path_kg_m2,
            precipitation_rate_mm_h: frame.weather.precipitation.rate_mm_per_hour,
        }
    }

    #[inline]
    pub(crate) fn from_cell(world_time_seconds: f64, cell: &EnvironmentAtmosphereCellDto) -> Self {
        Self {
            world_time_seconds,
            pressure_hpa: cell.atmosphere.surface_pressure_hpa,
            temperature_c: cell.atmosphere.temperature_celsius,
            specific_humidity: cell.atmosphere.specific_humidity_g_per_kg * 0.001,
            aerosol_mass: cell.atmosphere.aerosol_density,
            cloud_water_path_kg_m2: cell.atmosphere.cloud_water_path_kg_m2,
            precipitation_rate_mm_h: cell.weather.precipitation.rate_mm_per_hour,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BoundaryState {
    pub dt_seconds: f32,
    pub local_climate_temperature_c: f32,
    pub pressure_target_hpa: f32,
    pub equilibrium_specific_humidity: f32,
    pub net_radiative_flux_w_m2: f32,
    pub evaporation_flux_kg_m2_s: f32,
    pub aerosol_emission_per_s: f32,
    pub boundary_layer_depth_m: f32,
    pub boundary_layer_heat_capacity_j_m2_k: f32,
    pub geostrophic_wind: Vec3Dto,
    pub geostrophic_wind_mps: f32,
    pub surface_roughness_m: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PrognosticState {
    pub dt_seconds: f32,
    pub pressure_hpa: f32,
    pub temperature_c: f32,
    pub specific_humidity: f32,
    pub aerosol_mass: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ThermodynamicState {
    pub pressure_hpa: f32,
    pub temperature_c: f32,
    pub specific_humidity: f32,
    pub vapor_pressure_hpa: f32,
    pub saturation_vapor_pressure_hpa: f32,
    pub relative_humidity: f32,
    pub dew_point_c: f32,
    pub wet_bulb_c: f32,
    pub air_density_kg_m3: f32,
    pub lcl_m: f32,
    pub precipitable_water_mm: f32,
    pub supersaturation_condensate_kg_m2: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct VerticalState {
    pub layers: [AtmosphericLayerDto; 5],
    pub cape_j_kg: f32,
    pub cin_j_kg: f32,
    pub cloud_top_m: f32,
    pub upper_ice_transport: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MicrophysicsState {
    pub cloud_water_path_kg_m2: f32,
    pub cloud_coverage: f32,
    pub overcast: f32,
    pub precipitation_kind: PrecipitationKind,
    pub precipitation_rate_mm_h: f32,
    pub precipitation_intensity: f32,
    pub thunder_probability: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct OpticalState {
    pub aerosol_density: f32,
    pub haze_amount: f32,
    pub fog_density: f32,
    pub visibility_m: f32,
}
