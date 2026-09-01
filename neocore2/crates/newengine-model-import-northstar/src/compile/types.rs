use super::*;

#[derive(Clone, Debug)]
pub struct PackageSkinSubsetRule {
    pub package_path: PathBuf,
    pub source_domain_size: usize,
    pub local_to_master: Vec<Option<u16>>,
}

#[derive(Clone, Debug)]
pub struct CharacterCompileRequest {
    pub name: String,
    pub package_paths: Vec<PathBuf>,
    pub skeleton_path: PathBuf,
    pub skeleton_profile: SkeletonProfile,
    pub output_dir: PathBuf,
    /// Optional canonical NEMAT reference. When set, imported LOD0 meshes are
    /// bound deterministically as @m00, @m01, ... in package/mesh order.
    pub material_library_ref: Option<String>,
    /// Resolve default material slots from native source-material identity.
    pub material_by_source_identity: bool,
    /// Optional canonical source-material identity -> NEMAT slot assignments. When non-empty,
    /// every imported source material must be present here and the assigned slot is preserved
    /// across character outfit variants instead of being re-numbered from the variant subset.
    pub material_identity_slots: Vec<(String, usize)>,
    /// Optional per-package mesh prefixes. If a package has one or more entries here,
    /// only meshes whose decoded name starts with one of those prefixes are imported.
    pub package_mesh_prefixes: Vec<(PathBuf, String)>,
    /// Optional mesh-prefix to canonical material-ref overrides. Longest prefix wins.
    pub material_overrides: Vec<(String, String)>,
    /// Build-time completeness contract: every prefix must match at least one imported LOD0 mesh.
    pub required_mesh_prefixes: Vec<String>,
    /// Mesh prefixes whose native corrective/helper skin influences must be collapsed onto the
    /// canonical deform skeleton. NorthStar helper/twist branches are constraint-driven in the source
    /// runtime; leaving those joints at bind local while sparse locomotion animates the deform
    /// chain causes neighbouring vertices to diverge into visible rubber spikes.
    pub corrective_skin_collapse_prefixes: Vec<String>,
    /// Explicit fallback for packages whose skin domain is not the master skeleton. The listed
    /// master joints are used to produce a stable proximity-weighted skeletal approximation until
    /// the source cloth simulation-node domain has a dedicated runtime.
    pub package_skin_fallback_joints: Vec<(PathBuf, Vec<String>)>,
    /// Exact native master-rig mode. Source skin domains equal to the decoded skeleton are
    /// preserved verbatim; other domains require an explicit package subset mapping.
    pub master_rig: bool,
    /// Exact source-local -> master joint mappings, scoped by package and source domain.
    pub package_skin_subsets: Vec<PackageSkinSubsetRule>,
    /// Optional rigid affine transform from decoded PAK source space into canonical model space.
    /// The same matrix is persisted as YDD `skin_source_to_model`, preserving native skinning.
    pub source_to_model: Option<[f32; 16]>,
}

#[derive(Clone, Debug)]
pub struct CharacterCompileReport {
    pub ydd_path: PathBuf,
    pub ymt_path: PathBuf,
    pub mesh_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub joint_count: usize,
    pub skin_loss: SkinLossStats,
    pub material_slots: Vec<(String, String)>,
    pub skin_fallbacks: Vec<SkinFallbackReport>,
}

#[derive(Clone, Debug)]
pub struct SkinFallbackReport {
    pub package: PathBuf,
    pub mesh: String,
    pub source_joint_domain_size: usize,
    pub target_joints: Vec<String>,
}

/// Offline extraction of rigid pieces authored as joints inside one skinned NorthStar PC geometry.
/// This is used for weapon debris such as the five `rifle-shell-group` casing variants: source
/// skinning is consumed by the importer and runtime receives ordinary rigid YDD entries.
#[derive(Clone, Debug)]
pub struct RigidJointVariantsCompileRequest {
    pub name: String,
    pub package_path: PathBuf,
    pub joints: Vec<String>,
    pub output_path: PathBuf,
    pub material_ref: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RigidJointVariantsCompileReport {
    pub ydd_path: PathBuf,
    pub entry_count: usize,
    pub mesh_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
}

/// Offline compile request for an authored rigid/static NorthStar PC geometry package.
#[derive(Clone, Debug)]
pub struct StaticPakCompileRequest {
    pub name: String,
    pub package_path: PathBuf,
    pub output_path: PathBuf,
    pub material_ref: Option<String>,
    /// Explicitly allow a package-local skin stream to be baked at its decoded bind pose.
    /// This is intended for rigid props/weapons whose hierarchy is not a runtime dependency.
    pub bake_skinned_bind_pose: bool,
    /// Optional rigid transform from decoded source space to canonical model space.
    pub source_to_model: Option<[f32; 16]>,
}

#[derive(Clone, Debug)]
pub struct StaticPakCompileReport {
    pub ydd_path: PathBuf,
    pub mesh_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub bind_pose_baked: bool,
}
