use std::os::raw::{c_char, c_int, c_uint, c_void};

pub const JPC_PI: f32 = 3.14159265358979323846f32;
pub const JPC_MAX_PHYSICS_JOBS: c_int = 2048;
pub const JPC_MAX_PHYSICS_BARRIERS: c_int = 8;


// JoltC enum aliases intentionally kept as primitive C ABI-compatible
// aliases. The checked-in bindings are included inside `generated.rs`;
// crate/module-level allow attributes live there, not in this generated body.
pub type JPC_PhysicsUpdateError = u32;
pub const JPC_PHYSICS_UPDATE_ERROR_NONE: JPC_PhysicsUpdateError = 0;
pub const JPC_PHYSICS_UPDATE_ERROR_MANIFOLD_CACHE_FULL: JPC_PhysicsUpdateError = 1 << 0;
pub const JPC_PHYSICS_UPDATE_ERROR_BODY_PAIR_CACHE_FULL: JPC_PhysicsUpdateError = 1 << 1;
pub const JPC_PHYSICS_UPDATE_ERROR_CONTACT_CONSTRAINTS_FULL: JPC_PhysicsUpdateError = 1 << 2;

pub type JPC_ShapeColor = c_int;
pub const JPC_SHAPE_COLOR_INSTANCE_COLOR: JPC_ShapeColor = 0;
pub const JPC_SHAPE_COLOR_SHAPE_TYPE_COLOR: JPC_ShapeColor = 1;
pub const JPC_SHAPE_COLOR_MOTION_TYPE_COLOR: JPC_ShapeColor = 2;
pub const JPC_SHAPE_COLOR_SLEEP_COLOR: JPC_ShapeColor = 3;
pub const JPC_SHAPE_COLOR_ISLAND_COLOR: JPC_ShapeColor = 4;
pub const JPC_SHAPE_COLOR_MATERIAL_COLOR: JPC_ShapeColor = 5;

pub type JPC_ShapeType = u8;
pub const JPC_SHAPE_TYPE_CONVEX: JPC_ShapeType = 0;
pub const JPC_SHAPE_TYPE_COMPOUND: JPC_ShapeType = 1;
pub const JPC_SHAPE_TYPE_DECORATED: JPC_ShapeType = 2;
pub const JPC_SHAPE_TYPE_MESH: JPC_ShapeType = 3;
pub const JPC_SHAPE_TYPE_HEIGHT_FIELD: JPC_ShapeType = 4;
pub const JPC_SHAPE_TYPE_SOFTBODY: JPC_ShapeType = 5;
pub const JPC_SHAPE_TYPE_USER1: JPC_ShapeType = 6;
pub const JPC_SHAPE_TYPE_USER2: JPC_ShapeType = 7;
pub const JPC_SHAPE_TYPE_USER3: JPC_ShapeType = 8;
pub const JPC_SHAPE_TYPE_USER4: JPC_ShapeType = 9;

pub type JPC_ShapeSubType = u8;
pub const JPC_SHAPE_SUB_TYPE_SPHERE: JPC_ShapeSubType = 0;
pub const JPC_SHAPE_SUB_TYPE_BOX: JPC_ShapeSubType = 1;
pub const JPC_SHAPE_SUB_TYPE_TRIANGLE: JPC_ShapeSubType = 2;
pub const JPC_SHAPE_SUB_TYPE_CAPSULE: JPC_ShapeSubType = 3;
pub const JPC_SHAPE_SUB_TYPE_TAPEREDCAPSULE: JPC_ShapeSubType = 4;
pub const JPC_SHAPE_SUB_TYPE_CYLINDER: JPC_ShapeSubType = 5;
pub const JPC_SHAPE_SUB_TYPE_CONVEX_HULL: JPC_ShapeSubType = 6;
pub const JPC_SHAPE_SUB_TYPE_STATIC_COMPOUND: JPC_ShapeSubType = 7;
pub const JPC_SHAPE_SUB_TYPE_MUTABLE_COMPOUND: JPC_ShapeSubType = 8;
pub const JPC_SHAPE_SUB_TYPE_ROTATED_TRANSLATED: JPC_ShapeSubType = 9;
pub const JPC_SHAPE_SUB_TYPE_SCALED: JPC_ShapeSubType = 10;
pub const JPC_SHAPE_SUB_TYPE_OFFSET_CENTER_OF_MASS: JPC_ShapeSubType = 11;
pub const JPC_SHAPE_SUB_TYPE_MESH: JPC_ShapeSubType = 12;
pub const JPC_SHAPE_SUB_TYPE_HEIGHT_FIELD: JPC_ShapeSubType = 13;
pub const JPC_SHAPE_SUB_TYPE_SOFT_BODY: JPC_ShapeSubType = 14;
pub const JPC_SHAPE_SUB_TYPE_USER1: JPC_ShapeSubType = 15;
pub const JPC_SHAPE_SUB_TYPE_USER2: JPC_ShapeSubType = 16;
pub const JPC_SHAPE_SUB_TYPE_USER3: JPC_ShapeSubType = 17;
pub const JPC_SHAPE_SUB_TYPE_USER4: JPC_ShapeSubType = 18;
pub const JPC_SHAPE_SUB_TYPE_USER5: JPC_ShapeSubType = 19;
pub const JPC_SHAPE_SUB_TYPE_USER6: JPC_ShapeSubType = 20;
pub const JPC_SHAPE_SUB_TYPE_USER7: JPC_ShapeSubType = 21;
pub const JPC_SHAPE_SUB_TYPE_USER8: JPC_ShapeSubType = 22;
pub const JPC_SHAPE_SUB_TYPE_USER_CONVEX1: JPC_ShapeSubType = 23;
pub const JPC_SHAPE_SUB_TYPE_USER_CONVEX2: JPC_ShapeSubType = 24;
pub const JPC_SHAPE_SUB_TYPE_USER_CONVEX3: JPC_ShapeSubType = 25;
pub const JPC_SHAPE_SUB_TYPE_USER_CONVEX4: JPC_ShapeSubType = 26;
pub const JPC_SHAPE_SUB_TYPE_USER_CONVEX5: JPC_ShapeSubType = 27;
pub const JPC_SHAPE_SUB_TYPE_USER_CONVEX6: JPC_ShapeSubType = 28;
pub const JPC_SHAPE_SUB_TYPE_USER_CONVEX7: JPC_ShapeSubType = 29;
pub const JPC_SHAPE_SUB_TYPE_USER_CONVEX8: JPC_ShapeSubType = 30;

pub type JPC_ConstraintType = u32;
pub const JPC_CONSTRAINT_TYPE_CONSTRAINT: JPC_ConstraintType = 0;
pub const JPC_CONSTRAINT_TYPE_TWO_BODY_CONSTRAINT: JPC_ConstraintType = 1;

pub type JPC_ConstraintSubType = u32;
pub const JPC_CONSTRAINT_SUB_TYPE_FIXED: JPC_ConstraintSubType = 0;
pub const JPC_CONSTRAINT_SUB_TYPE_POINT: JPC_ConstraintSubType = 1;
pub const JPC_CONSTRAINT_SUB_TYPE_HINGE: JPC_ConstraintSubType = 2;
pub const JPC_CONSTRAINT_SUB_TYPE_SLIDER: JPC_ConstraintSubType = 3;
pub const JPC_CONSTRAINT_SUB_TYPE_DISTANCE: JPC_ConstraintSubType = 4;
pub const JPC_CONSTRAINT_SUB_TYPE_CONE: JPC_ConstraintSubType = 5;
pub const JPC_CONSTRAINT_SUB_TYPE_SWING_TWIST: JPC_ConstraintSubType = 6;
pub const JPC_CONSTRAINT_SUB_TYPE_SIX_DOF: JPC_ConstraintSubType = 7;
pub const JPC_CONSTRAINT_SUB_TYPE_PATH: JPC_ConstraintSubType = 8;
pub const JPC_CONSTRAINT_SUB_TYPE_VEHICLE: JPC_ConstraintSubType = 9;
pub const JPC_CONSTRAINT_SUB_TYPE_RACK_AND_PINION: JPC_ConstraintSubType = 10;
pub const JPC_CONSTRAINT_SUB_TYPE_GEAR: JPC_ConstraintSubType = 11;
pub const JPC_CONSTRAINT_SUB_TYPE_PULLEY: JPC_ConstraintSubType = 12;
pub const JPC_CONSTRAINT_SUB_TYPE_USER1: JPC_ConstraintSubType = 13;
pub const JPC_CONSTRAINT_SUB_TYPE_USER2: JPC_ConstraintSubType = 14;
pub const JPC_CONSTRAINT_SUB_TYPE_USER3: JPC_ConstraintSubType = 15;
pub const JPC_CONSTRAINT_SUB_TYPE_USER4: JPC_ConstraintSubType = 16;

pub type JPC_ConstraintSpace = u32;
pub const JPC_CONSTRAINT_SPACE_LOCAL_TO_BODY_COM: JPC_ConstraintSpace = 0;
pub const JPC_CONSTRAINT_SPACE_WORLD_SPACE: JPC_ConstraintSpace = 1;

pub type JPC_MotionType = u8;
pub const JPC_MOTION_TYPE_STATIC: JPC_MotionType = 0;
pub const JPC_MOTION_TYPE_KINEMATIC: JPC_MotionType = 1;
pub const JPC_MOTION_TYPE_DYNAMIC: JPC_MotionType = 2;

pub type JPC_MotionQuality = u8;
pub const JPC_MOTION_QUALITY_DISCRETE: JPC_MotionQuality = 0;
pub const JPC_MOTION_QUALITY_LINEAR_CAST: JPC_MotionQuality = 1;

pub type JPC_OverrideMassProperties = u8;
pub const JPC_OVERRIDE_MASS_PROPS_CALC_MASS_INERTIA: JPC_OverrideMassProperties = 0;
pub const JPC_OVERRIDE_MASS_PROPS_CALC_INERTIA: JPC_OverrideMassProperties = 1;
pub const JPC_OVERRIDE_MASS_PROPS_MASS_INERTIA_PROVIDED: JPC_OverrideMassProperties = 2;

pub type JPC_GroundState = u32;
pub const JPC_CHARACTER_GROUND_STATE_ON_GROUND: JPC_GroundState = 0;
pub const JPC_CHARACTER_GROUND_STATE_ON_STEEP_GROUND: JPC_GroundState = 1;
pub const JPC_CHARACTER_GROUND_STATE_NOT_SUPPORTED: JPC_GroundState = 2;
pub const JPC_CHARACTER_GROUND_STATE_IN_AIR: JPC_GroundState = 3;

pub type JPC_Activation = u32;
pub const JPC_ACTIVATION_ACTIVATE: JPC_Activation = 0;
pub const JPC_ACTIVATION_DONT_ACTIVATE: JPC_Activation = 1;

pub type JPC_ValidateResult = u32;
pub const JPC_VALIDATE_RESULT_ACCEPT_ALL_CONTACTS: JPC_ValidateResult = 0;
pub const JPC_VALIDATE_RESULT_ACCEPT_CONTACT: JPC_ValidateResult = 1;
pub const JPC_VALIDATE_RESULT_REJECT_CONTACT: JPC_ValidateResult = 2;
pub const JPC_VALIDATE_RESULT_REJECT_ALL_CONTACTS: JPC_ValidateResult = 3;

pub type JPC_BackFaceMode = u8;
pub const JPC_BACK_FACE_IGNORE: JPC_BackFaceMode = 0;
pub const JPC_BACK_FACE_COLLIDE: JPC_BackFaceMode = 1;

pub type JPC_BodyType = u8;
pub const JPC_BODY_TYPE_RIGID_BODY: JPC_BodyType = 0;
pub const JPC_BODY_TYPE_SOFT_BODY: JPC_BodyType = 1;

pub type JPC_AllowedDOFs = u8;
pub const JPC_ALLOWED_DOFS_NONE: JPC_AllowedDOFs = 0;
pub const JPC_ALLOWED_DOFS_ALL: JPC_AllowedDOFs = 63;
pub const JPC_ALLOWED_DOFS_TRANSLATIONX: JPC_AllowedDOFs = 1;
pub const JPC_ALLOWED_DOFS_TRANSLATIONY: JPC_AllowedDOFs = 2;
pub const JPC_ALLOWED_DOFS_TRANSLATIONZ: JPC_AllowedDOFs = 4;
pub const JPC_ALLOWED_DOFS_ROTATIONX: JPC_AllowedDOFs = 8;
pub const JPC_ALLOWED_DOFS_ROTATIONY: JPC_AllowedDOFs = 16;
pub const JPC_ALLOWED_DOFS_ROTATIONZ: JPC_AllowedDOFs = 32;
pub const JPC_ALLOWED_DOFS_PLANE2D: JPC_AllowedDOFs = 35;

pub type JPC_Features = u32;
pub const JPC_FEATURE_DOUBLE_PRECISION: JPC_Features = 1;
pub const JPC_FEATURE_NEON: JPC_Features = 2;
pub const JPC_FEATURE_SSE: JPC_Features = 4;
pub const JPC_FEATURE_SSE4_1: JPC_Features = 8;
pub const JPC_FEATURE_SSE4_2: JPC_Features = 16;
pub const JPC_FEATURE_AVX: JPC_Features = 32;
pub const JPC_FEATURE_AVX2: JPC_Features = 64;
pub const JPC_FEATURE_AVX512: JPC_Features = 128;
pub const JPC_FEATURE_F16C: JPC_Features = 256;
pub const JPC_FEATURE_LZCNT: JPC_Features = 512;
pub const JPC_FEATURE_TZCNT: JPC_Features = 1024;
pub const JPC_FEATURE_FMADD: JPC_Features = 2048;
pub const JPC_FEATURE_PLATFORM_DETERMINISTIC: JPC_Features = 4096;
pub const JPC_FEATURE_FLOATING_POINT_EXCEPTIONS: JPC_Features = 8192;
pub const JPC_FEATURE_DEBUG: JPC_Features = 16384;

pub type JPC_BodyID = u32;
pub type JPC_SubShapeID = u32;
pub type JPC_BroadPhaseLayer = u8;
#[cfg(feature = "object-layer-u32")] pub type JPC_ObjectLayer = u32;
#[cfg(not(feature = "object-layer-u32"))] pub type JPC_ObjectLayer = u16;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_Float3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct JPC_Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub _w: f32,
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct JPC_Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C, align(32))]
#[derive(Debug, Copy, Clone)]
pub struct JPC_DVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub _w: f64,
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct JPC_Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct JPC_Mat44 {
    pub matrix: [JPC_Vec4; 4],
}

#[repr(C, align(32))]
#[derive(Debug, Copy, Clone)]
pub struct JPC_DMat44 {
    pub col: [JPC_Vec4; 3],
    pub col3: JPC_DVec3,
}

#[repr(C, align(4))]
#[derive(Debug, Copy, Clone)]
pub struct JPC_Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_IndexedTriangleNoMaterial {
    pub idx: [u32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_IndexedTriangle {
    pub idx: [u32; 3],
    pub materialIndex: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_RayCast {
    pub Origin: JPC_Vec3,
    pub Direction: JPC_Vec3,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_RRayCast {
    pub Origin: JPC_RVec3,
    pub Direction: JPC_Vec3,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_RayCastResult {
    pub BodyID: JPC_BodyID,
    pub Fraction: f32,
    pub SubShapeID2: JPC_SubShapeID,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_BroadPhaseLayerInterfaceFns {
    pub GetNumBroadPhaseLayers: Option<unsafe extern "C" fn(*const c_void) -> c_uint>, 
    pub GetBroadPhaseLayer: Option<unsafe extern "C" fn(*const c_void, JPC_ObjectLayer) -> JPC_BroadPhaseLayer>, 
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_BroadPhaseLayerFilterFns {
    pub ShouldCollide: Option<unsafe extern "C" fn(*const c_void, JPC_BroadPhaseLayer) -> bool>, 
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_ObjectLayerFilterFns {
    pub ShouldCollide: Option<unsafe extern "C" fn(*const c_void, JPC_ObjectLayer) -> bool>, 
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_BodyFilterFns {
    pub ShouldCollide: Option<unsafe extern "C" fn(*const c_void, JPC_BodyID) -> bool>, 
    pub ShouldCollideLocked: Option<unsafe extern "C" fn(*const c_void, *const JPC_Body) -> bool>, 
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_ObjectVsBroadPhaseLayerFilterFns {
    pub ShouldCollide: Option<unsafe extern "C" fn(*const c_void, JPC_ObjectLayer, JPC_BroadPhaseLayer) -> bool>, 
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_ObjectLayerPairFilterFns {
    pub ShouldCollide: Option<unsafe extern "C" fn(*const c_void, JPC_ObjectLayer, JPC_ObjectLayer) -> bool>, 
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_BodyManager_DrawSettings {
    pub mDrawGetSupportFunction: bool,
    pub mDrawSupportDirection: bool,
    pub mDrawGetSupportingFace: bool,
    pub mDrawShape: bool,
    pub mDrawShapeWireframe: bool,
    pub mDrawShapeColor: JPC_ShapeColor,
    pub mDrawBoundingBox: bool,
    pub mDrawCenterOfMassTransform: bool,
    pub mDrawWorldTransform: bool,
    pub mDrawVelocity: bool,
    pub mDrawMassAndInertia: bool,
    pub mDrawSleepStats: bool,
    pub mDrawSoftBodyVertices: bool,
    pub mDrawSoftBodyVertexVelocities: bool,
    pub mDrawSoftBodyEdgeConstraints: bool,
    pub mDrawSoftBodyBendConstraints: bool,
    pub mDrawSoftBodyVolumeConstraints: bool,
    pub mDrawSoftBodySkinConstraints: bool,
    pub mDrawSoftBodyLRAConstraints: bool,
    pub mDrawSoftBodyPredictedBounds: bool,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_DebugRendererSimpleFns {
    pub DrawLine: Option<unsafe extern "C" fn(*const c_void, JPC_RVec3, JPC_RVec3, JPC_Color)>, 
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_TriangleShapeSettings {
    pub UserData: u64,
    pub Density: f32,
    pub V1: JPC_Vec3,
    pub V2: JPC_Vec3,
    pub V3: JPC_Vec3,
    pub ConvexRadius: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_BoxShapeSettings {
    pub UserData: u64,
    pub Density: f32,
    pub HalfExtent: JPC_Vec3,
    pub ConvexRadius: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_SphereShapeSettings {
    pub UserData: u64,
    pub Density: f32,
    pub Radius: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_CapsuleShapeSettings {
    pub UserData: u64,
    pub Density: f32,
    pub Radius: f32,
    pub HalfHeightOfCylinder: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_CylinderShapeSettings {
    pub UserData: u64,
    pub Density: f32,
    pub HalfHeight: f32,
    pub Radius: f32,
    pub ConvexRadius: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_ConvexHullShapeSettings {
    pub UserData: u64,
    pub Density: f32,
    pub Points: *const JPC_Vec3,
    pub PointsLen: usize,
    pub MaxConvexRadius: f32,
    pub MaxErrorConvexRadius: f32,
    pub HullTolerance: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_MeshShapeSettings {
    pub UserData: u64,
    pub Vertices: *const JPC_Float3,
    pub VerticesLen: usize,
    pub Triangles: *const JPC_IndexedTriangle,
    pub TrianglesLen: usize,
    pub MaxTrianglesPerLeaf: c_uint,
    pub ActiveEdgeCosThresholdAngle: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_HeightFieldShapeSettings {
    pub UserData: u64,
    pub Samples: *const f32,
    pub SampleCount: u32,
    pub Offset: JPC_Vec3,
    pub Scale: JPC_Vec3,
    pub MinHeightValue: f32,
    pub MaxHeightValue: f32,
    pub BlockSize: u32,
    pub BitsPerSample: u32,
    pub ActiveEdgeCosThresholdAngle: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_SubShapeSettings {
    pub Shape: *const JPC_Shape,
    pub Position: JPC_Vec3,
    pub Rotation: JPC_Quat,
    pub UserData: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_StaticCompoundShapeSettings {
    pub UserData: u64,
    pub SubShapes: *const JPC_SubShapeSettings,
    pub SubShapesLen: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_MutableCompoundShapeSettings {
    pub UserData: u64,
    pub SubShapes: *const JPC_SubShapeSettings,
    pub SubShapesLen: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_BodyCreationSettings {
    pub Position: JPC_RVec3,
    pub Rotation: JPC_Quat,
    pub LinearVelocity: JPC_Vec3,
    pub AngularVelocity: JPC_Vec3,
    pub UserData: u64,
    pub ObjectLayer: JPC_ObjectLayer,
    pub MotionType: JPC_MotionType,
    pub AllowedDOFs: JPC_AllowedDOFs,
    pub AllowDynamicOrKinematic: bool,
    pub IsSensor: bool,
    pub CollideKinematicVsNonDynamic: bool,
    pub UseManifoldReduction: bool,
    pub ApplyGyroscopicForce: bool,
    pub MotionQuality: JPC_MotionQuality,
    pub EnhancedInternalEdgeRemoval: bool,
    pub AllowSleeping: bool,
    pub Friction: f32,
    pub Restitution: f32,
    pub LinearDamping: f32,
    pub AngularDamping: f32,
    pub MaxLinearVelocity: f32,
    pub MaxAngularVelocity: f32,
    pub GravityFactor: f32,
    pub NumVelocityStepsOverride: c_uint,
    pub NumPositionStepsOverride: c_uint,
    pub OverrideMassProperties: JPC_OverrideMassProperties,
    pub InertiaMultiplier: f32,
    pub Shape: *const JPC_Shape,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_NarrowPhaseQuery_CastRayArgs {
    pub Ray: JPC_RRayCast,
    pub Result: JPC_RayCastResult,
    pub BroadPhaseLayerFilter: *const JPC_BroadPhaseLayerFilter,
    pub ObjectLayerFilter: *const JPC_ObjectLayerFilter,
    pub BodyFilter: *const JPC_BodyFilter,
}

#[cfg(feature = "double-precision")] pub type JPC_RVec3 = JPC_DVec3;
#[cfg(not(feature = "double-precision"))] pub type JPC_RVec3 = JPC_Vec3;
#[cfg(feature = "double-precision")] pub type JPC_RMat44 = JPC_DMat44;
#[cfg(not(feature = "double-precision"))] pub type JPC_RMat44 = JPC_Mat44;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_ContactEvent {
    pub Body1ID: JPC_BodyID,
    pub Body2ID: JPC_BodyID,
    pub Body1UserData: u64,
    pub Body2UserData: u64,
    pub SubShapeID1: JPC_SubShapeID,
    pub SubShapeID2: JPC_SubShapeID,
    pub Point: JPC_RVec3,
    pub Normal: JPC_Vec3,
    pub PenetrationDepth: f32,
    pub EstimatedImpulse: f32,
}

pub type JPC_ContactEventCallback = Option<
    unsafe extern "C" fn(user_data: *mut c_void, event: *const JPC_ContactEvent),
>;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JPC_ContactListenerFns {
    pub OnContactAdded: JPC_ContactEventCallback,
    pub OnContactPersisted: JPC_ContactEventCallback,
}

#[repr(C)] pub struct JPC_Body { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_ContactListener { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_BodyFilter { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_BodyInterface { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_BroadPhaseLayerFilter { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_BroadPhaseLayerInterface { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_DebugRendererSimple { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_IndexedTriangleList { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_JobSystemThreadPool { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_NarrowPhaseQuery { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_ObjectLayerFilter { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_ObjectLayerPairFilter { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_ObjectVsBroadPhaseLayerFilter { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_PhysicsSystem { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_Shape { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_String { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_TempAllocatorImpl { _unused: [u8; 0] }
#[repr(C)] pub struct JPC_VertexList { _unused: [u8; 0] }

extern "C" {
    pub fn JPC_RegisterDefaultAllocator();
    pub fn JPC_FactoryInit();
    pub fn JPC_FactoryDelete();
    pub fn JPC_RegisterTypes();
    pub fn JPC_UnregisterTypes();
    pub fn JPC_ContactListener_new(
        user_data: *mut c_void,
        callbacks: JPC_ContactListenerFns,
    ) -> *mut JPC_ContactListener;
    pub fn JPC_ContactListener_delete(listener: *mut JPC_ContactListener);
    pub fn JPC_VertexList_new(arg0: *const JPC_Float3, arg1: usize) -> *mut JPC_VertexList;
    pub fn JPC_VertexList_delete(arg0: *mut JPC_VertexList);
    pub fn JPC_IndexedTriangleList_new(arg0: *const JPC_IndexedTriangle, arg1: usize) -> *mut JPC_IndexedTriangleList;
    pub fn JPC_IndexedTriangleList_delete(arg0: *mut JPC_IndexedTriangleList);
    pub fn JPC_TempAllocatorImpl_new(arg0: c_uint) -> *mut JPC_TempAllocatorImpl;
    pub fn JPC_TempAllocatorImpl_delete(arg0: *mut JPC_TempAllocatorImpl);
    pub fn JPC_JobSystemThreadPool_new2(arg0: c_uint, arg1: c_uint) -> *mut JPC_JobSystemThreadPool;
    pub fn JPC_JobSystemThreadPool_new3(arg0: c_uint, arg1: c_uint, arg2: c_int) -> *mut JPC_JobSystemThreadPool;
    pub fn JPC_JobSystemThreadPool_delete(arg0: *mut JPC_JobSystemThreadPool);
    pub fn JPC_BroadPhaseLayerInterface_new(arg0: *const c_void, arg1: JPC_BroadPhaseLayerInterfaceFns) -> *mut JPC_BroadPhaseLayerInterface;
    pub fn JPC_BroadPhaseLayerInterface_delete(arg0: *mut JPC_BroadPhaseLayerInterface);
    pub fn JPC_BroadPhaseLayerFilter_new(arg0: *const c_void, arg1: JPC_BroadPhaseLayerFilterFns) -> *mut JPC_BroadPhaseLayerFilter;
    pub fn JPC_BroadPhaseLayerFilter_delete(arg0: *mut JPC_BroadPhaseLayerFilter);
    pub fn JPC_ObjectLayerFilter_new(arg0: *const c_void, arg1: JPC_ObjectLayerFilterFns) -> *mut JPC_ObjectLayerFilter;
    pub fn JPC_ObjectLayerFilter_delete(arg0: *mut JPC_ObjectLayerFilter);
    pub fn JPC_BodyFilter_new(arg0: *const c_void, arg1: JPC_BodyFilterFns) -> *mut JPC_BodyFilter;
    pub fn JPC_BodyFilter_delete(arg0: *mut JPC_BodyFilter);
    pub fn JPC_ObjectVsBroadPhaseLayerFilter_new(arg0: *const c_void, arg1: JPC_ObjectVsBroadPhaseLayerFilterFns) -> *mut JPC_ObjectVsBroadPhaseLayerFilter;
    pub fn JPC_ObjectVsBroadPhaseLayerFilter_delete(arg0: *mut JPC_ObjectVsBroadPhaseLayerFilter);
    pub fn JPC_ObjectLayerPairFilter_new(arg0: *const c_void, arg1: JPC_ObjectLayerPairFilterFns) -> *mut JPC_ObjectLayerPairFilter;
    pub fn JPC_ObjectLayerPairFilter_delete(arg0: *mut JPC_ObjectLayerPairFilter);
    pub fn JPC_BodyManager_DrawSettings_default(arg0: *mut JPC_BodyManager_DrawSettings);
    pub fn JPC_DebugRendererSimple_new(arg0: *const c_void, arg1: JPC_DebugRendererSimpleFns) -> *mut JPC_DebugRendererSimple;
    pub fn JPC_DebugRendererSimple_delete(arg0: *mut JPC_DebugRendererSimple);
    pub fn JPC_String_delete(arg0: *mut JPC_String);
    pub fn JPC_String_c_str(arg0: *mut JPC_String) -> *const c_char;
    pub fn JPC_Shape_GetRefCount(arg0: *const JPC_Shape) -> u32;
    pub fn JPC_Shape_AddRef(arg0: *const JPC_Shape);
    pub fn JPC_Shape_Release(arg0: *const JPC_Shape);
    pub fn JPC_Shape_GetCenterOfMass(arg0: *const JPC_Shape) -> JPC_Vec3;
    pub fn JPC_TriangleShapeSettings_default(arg0: *mut JPC_TriangleShapeSettings);
    pub fn JPC_TriangleShapeSettings_Create(arg0: *const JPC_TriangleShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_BoxShapeSettings_default(arg0: *mut JPC_BoxShapeSettings);
    pub fn JPC_BoxShapeSettings_Create(arg0: *const JPC_BoxShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_SphereShapeSettings_default(arg0: *mut JPC_SphereShapeSettings);
    pub fn JPC_SphereShapeSettings_Create(arg0: *const JPC_SphereShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_CapsuleShapeSettings_default(arg0: *mut JPC_CapsuleShapeSettings);
    pub fn JPC_CapsuleShapeSettings_Create(arg0: *const JPC_CapsuleShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_CylinderShapeSettings_default(arg0: *mut JPC_CylinderShapeSettings);
    pub fn JPC_CylinderShapeSettings_Create(arg0: *const JPC_CylinderShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_ConvexHullShapeSettings_default(arg0: *mut JPC_ConvexHullShapeSettings);
    pub fn JPC_ConvexHullShapeSettings_Create(arg0: *const JPC_ConvexHullShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_MeshShapeSettings_default(arg0: *mut JPC_MeshShapeSettings);
    pub fn JPC_MeshShapeSettings_Create(arg0: *const JPC_MeshShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_HeightFieldShapeSettings_default(arg0: *mut JPC_HeightFieldShapeSettings);
    pub fn JPC_HeightFieldShapeSettings_Create(arg0: *const JPC_HeightFieldShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_SubShapeSettings_default(arg0: *mut JPC_SubShapeSettings);
    pub fn JPC_StaticCompoundShapeSettings_default(arg0: *mut JPC_StaticCompoundShapeSettings);
    pub fn JPC_StaticCompoundShapeSettings_Create(arg0: *const JPC_StaticCompoundShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_MutableCompoundShapeSettings_default(arg0: *mut JPC_MutableCompoundShapeSettings);
    pub fn JPC_MutableCompoundShapeSettings_Create(arg0: *const JPC_MutableCompoundShapeSettings, arg1: *mut *mut JPC_Shape, arg2: *mut *mut JPC_String) -> bool;
    pub fn JPC_BodyCreationSettings_default(arg0: *mut JPC_BodyCreationSettings);
    pub fn JPC_BodyCreationSettings_new() -> *mut JPC_BodyCreationSettings;
    pub fn JPC_Body_GetID(arg0: *const JPC_Body) -> JPC_BodyID;
    pub fn JPC_Body_GetBodyType(arg0: *const JPC_Body) -> JPC_BodyType;
    pub fn JPC_Body_IsRigidBody(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_IsSoftBody(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_IsActive(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_IsStatic(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_IsKinematic(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_IsDynamic(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_CanBeKinematicOrDynamic(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_SetIsSensor(arg0: *mut JPC_Body, arg1: bool);
    pub fn JPC_Body_IsSensor(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_SetCollideKinematicVsNonDynamic(arg0: *mut JPC_Body, arg1: bool);
    pub fn JPC_Body_GetCollideKinematicVsNonDynamic(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_SetUseManifoldReduction(arg0: *mut JPC_Body, arg1: bool);
    pub fn JPC_Body_GetUseManifoldReduction(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_GetUseManifoldReductionWithBody(arg0: *const JPC_Body, arg1: *const JPC_Body) -> bool;
    pub fn JPC_Body_SetApplyGyroscopicForce(arg0: *mut JPC_Body, arg1: bool);
    pub fn JPC_Body_GetApplyGyroscopicForce(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_SetEnhancedInternalEdgeRemoval(arg0: *mut JPC_Body, arg1: bool);
    pub fn JPC_Body_GetEnhancedInternalEdgeRemoval(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_GetEnhancedInternalEdgeRemovalWithBody(arg0: *const JPC_Body, arg1: *const JPC_Body) -> bool;
    pub fn JPC_Body_GetMotionType(arg0: *const JPC_Body) -> JPC_MotionType;
    pub fn JPC_Body_SetMotionType(arg0: *mut JPC_Body, arg1: JPC_MotionType);
    pub fn JPC_Body_GetBroadPhaseLayer(arg0: *const JPC_Body) -> JPC_BroadPhaseLayer;
    pub fn JPC_Body_GetObjectLayer(arg0: *const JPC_Body) -> JPC_ObjectLayer;
    pub fn JPC_Body_GetAllowSleeping(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_SetAllowSleeping(arg0: *mut JPC_Body, arg1: bool);
    pub fn JPC_Body_ResetSleepTimer(arg0: *mut JPC_Body);
    pub fn JPC_Body_GetFriction(arg0: *const JPC_Body) -> f32;
    pub fn JPC_Body_SetFriction(arg0: *mut JPC_Body, arg1: f32);
    pub fn JPC_Body_GetRestitution(arg0: *const JPC_Body) -> f32;
    pub fn JPC_Body_SetRestitution(arg0: *mut JPC_Body, arg1: f32);
    pub fn JPC_Body_GetLinearVelocity(arg0: *const JPC_Body) -> JPC_Vec3;
    pub fn JPC_Body_SetLinearVelocity(arg0: *mut JPC_Body, arg1: JPC_Vec3);
    pub fn JPC_Body_SetLinearVelocityClamped(arg0: *mut JPC_Body, arg1: JPC_Vec3);
    pub fn JPC_Body_GetAngularVelocity(arg0: *const JPC_Body) -> JPC_Vec3;
    pub fn JPC_Body_SetAngularVelocity(arg0: *mut JPC_Body, arg1: JPC_Vec3);
    pub fn JPC_Body_SetAngularVelocityClamped(arg0: *mut JPC_Body, arg1: JPC_Vec3);
    pub fn JPC_Body_GetPointVelocityCOM(arg0: *const JPC_Body, arg1: JPC_Vec3) -> JPC_Vec3;
    pub fn JPC_Body_GetPointVelocity(arg0: *const JPC_Body, arg1: JPC_RVec3) -> JPC_Vec3;
    pub fn JPC_Body_AddForce(arg0: *mut JPC_Body, arg1: JPC_Vec3);
    pub fn JPC_Body_AddTorque(arg0: *mut JPC_Body, arg1: JPC_Vec3);
    pub fn JPC_Body_GetAccumulatedForce(arg0: *const JPC_Body) -> JPC_Vec3;
    pub fn JPC_Body_GetAccumulatedTorque(arg0: *const JPC_Body) -> JPC_Vec3;
    pub fn JPC_Body_ResetForce(arg0: *mut JPC_Body);
    pub fn JPC_Body_ResetTorque(arg0: *mut JPC_Body);
    pub fn JPC_Body_ResetMotion(arg0: *mut JPC_Body);
    pub fn JPC_Body_GetInverseInertia(arg0: *const JPC_Body, arg1: *mut JPC_Mat44);
    pub fn JPC_Body_AddImpulse(arg0: *mut JPC_Body, arg1: JPC_Vec3);
    pub fn JPC_Body_AddImpulse2(arg0: *mut JPC_Body, arg1: JPC_Vec3, arg2: JPC_RVec3);
    pub fn JPC_Body_AddAngularImpulse(arg0: *mut JPC_Body, arg1: JPC_Vec3);
    pub fn JPC_Body_MoveKinematic(arg0: *mut JPC_Body, arg1: JPC_RVec3, arg2: JPC_Quat, arg3: f32);
    pub fn JPC_Body_ApplyBuoyancyImpulse(arg0: *mut JPC_Body, arg1: JPC_RVec3, arg2: JPC_Vec3, arg3: f32, arg4: f32, arg5: f32, arg6: JPC_Vec3, arg7: JPC_Vec3, arg8: f32) -> bool;
    pub fn JPC_Body_IsInBroadPhase(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_IsCollisionCacheInvalid(arg0: *const JPC_Body) -> bool;
    pub fn JPC_Body_GetShape(arg0: *const JPC_Body) -> *const JPC_Shape;
    pub fn JPC_Body_GetPosition(arg0: *const JPC_Body) -> JPC_RVec3;
    pub fn JPC_Body_GetRotation(arg0: *const JPC_Body) -> JPC_Quat;
    pub fn JPC_Body_GetCenterOfMassPosition(arg0: *const JPC_Body) -> JPC_RVec3;
    pub fn JPC_Body_GetUserData(arg0: *const JPC_Body) -> u64;
    pub fn JPC_Body_SetUserData(arg0: *mut JPC_Body, arg1: u64);
    pub fn JPC_BodyInterface_CreateBody(arg0: *mut JPC_BodyInterface, arg1: *const JPC_BodyCreationSettings) -> *mut JPC_Body;
    pub fn JPC_BodyInterface_CreateBodyWithID(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: *const JPC_BodyCreationSettings) -> *mut JPC_Body;
    pub fn JPC_BodyInterface_CreateBodyWithoutID(arg0: *const JPC_BodyInterface, arg1: *const JPC_BodyCreationSettings) -> *mut JPC_Body;
    pub fn JPC_BodyInterface_DestroyBodyWithoutID(arg0: *const JPC_BodyInterface, arg1: *mut JPC_Body);
    pub fn JPC_BodyInterface_AssignBodyID(arg0: *mut JPC_BodyInterface, arg1: *mut JPC_Body) -> bool;
    pub fn JPC_BodyInterface_UnassignBodyID(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID) -> *mut JPC_Body;
    pub fn JPC_BodyInterface_UnassignBodyIDs(arg0: *mut JPC_BodyInterface, arg1: *const JPC_BodyID, arg2: c_int, arg3: *mut *mut JPC_Body);
    pub fn JPC_BodyInterface_DestroyBody(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID);
    pub fn JPC_BodyInterface_DestroyBodies(arg0: *mut JPC_BodyInterface, arg1: *const JPC_BodyID, arg2: c_int);
    pub fn JPC_BodyInterface_AddBody(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Activation);
    pub fn JPC_BodyInterface_RemoveBody(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID);
    pub fn JPC_BodyInterface_IsAdded(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> bool;
    pub fn JPC_BodyInterface_CreateAndAddBody(arg0: *mut JPC_BodyInterface, arg1: *const JPC_BodyCreationSettings, arg2: JPC_Activation) -> JPC_BodyID;
    pub fn JPC_BodyInterface_AddBodiesPrepare(arg0: *mut JPC_BodyInterface, arg1: *mut JPC_BodyID, arg2: c_int) -> *mut c_void;
    pub fn JPC_BodyInterface_AddBodiesFinalize(arg0: *mut JPC_BodyInterface, arg1: *mut JPC_BodyID, arg2: c_int, arg3: *mut c_void, arg4: JPC_Activation);
    pub fn JPC_BodyInterface_AddBodiesAbort(arg0: *mut JPC_BodyInterface, arg1: *mut JPC_BodyID, arg2: c_int, arg3: *mut c_void);
    pub fn JPC_BodyInterface_RemoveBodies(arg0: *mut JPC_BodyInterface, arg1: *mut JPC_BodyID, arg2: c_int);
    pub fn JPC_BodyInterface_ActivateBody(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID);
    pub fn JPC_BodyInterface_ActivateBodies(arg0: *mut JPC_BodyInterface, arg1: *mut JPC_BodyID, arg2: c_int);
    pub fn JPC_BodyInterface_DeactivateBody(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID);
    pub fn JPC_BodyInterface_DeactivateBodies(arg0: *mut JPC_BodyInterface, arg1: *mut JPC_BodyID, arg2: c_int);
    pub fn JPC_BodyInterface_IsActive(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> bool;
    pub fn JPC_BodyInterface_SetShape(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID, arg2: *const JPC_Shape, arg3: bool, arg4: JPC_Activation);
    pub fn JPC_BodyInterface_NotifyShapeChanged(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3, arg3: bool, arg4: JPC_Activation);
    pub fn JPC_BodyInterface_SetObjectLayer(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_ObjectLayer);
    pub fn JPC_BodyInterface_GetObjectLayer(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_ObjectLayer;
    pub fn JPC_BodyInterface_SetPositionAndRotation(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_RVec3, arg3: JPC_Quat, arg4: JPC_Activation);
    pub fn JPC_BodyInterface_SetPositionAndRotationWhenChanged(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_RVec3, arg3: JPC_Quat, arg4: JPC_Activation);
    pub fn JPC_BodyInterface_GetPositionAndRotation(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID, arg2: *mut JPC_RVec3, arg3: *mut JPC_Quat);
    pub fn JPC_BodyInterface_SetPosition(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_RVec3, arg3: JPC_Activation);
    pub fn JPC_BodyInterface_GetPosition(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_RVec3;
    pub fn JPC_BodyInterface_GetCenterOfMassPosition(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_RVec3;
    pub fn JPC_BodyInterface_SetRotation(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Quat, arg3: JPC_Activation);
    pub fn JPC_BodyInterface_GetRotation(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_Quat;
    pub fn JPC_BodyInterface_MoveKinematic(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_RVec3, arg3: JPC_Quat, arg4: f32);
    pub fn JPC_BodyInterface_SetLinearAndAngularVelocity(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3, arg3: JPC_Vec3);
    pub fn JPC_BodyInterface_GetLinearAndAngularVelocity(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID, arg2: *mut JPC_Vec3, arg3: *mut JPC_Vec3);
    pub fn JPC_BodyInterface_SetLinearVelocity(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3);
    pub fn JPC_BodyInterface_GetLinearVelocity(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_Vec3;
    pub fn JPC_BodyInterface_AddLinearVelocity(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3);
    pub fn JPC_BodyInterface_AddLinearAndAngularVelocity(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3, arg3: JPC_Vec3);
    pub fn JPC_BodyInterface_SetAngularVelocity(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3);
    pub fn JPC_BodyInterface_GetAngularVelocity(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_Vec3;
    pub fn JPC_BodyInterface_GetPointVelocity(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_RVec3) -> JPC_Vec3;
    pub fn JPC_BodyInterface_SetPositionRotationAndVelocity(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_RVec3, arg3: JPC_Quat, arg4: JPC_Vec3, arg5: JPC_Vec3);
    pub fn JPC_BodyInterface_AddForce(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3);
    pub fn JPC_BodyInterface_AddTorque(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3);
    pub fn JPC_BodyInterface_AddForceAndTorque(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3, arg3: JPC_Vec3);
    pub fn JPC_BodyInterface_AddImpulse(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3);
    pub fn JPC_BodyInterface_AddImpulse3(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3, arg3: JPC_RVec3);
    pub fn JPC_BodyInterface_AddAngularImpulse(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_Vec3);
    pub fn JPC_BodyInterface_GetBodyType(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_BodyType;
    pub fn JPC_BodyInterface_SetMotionType(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_MotionType, arg3: JPC_Activation);
    pub fn JPC_BodyInterface_GetMotionType(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_MotionType;
    pub fn JPC_BodyInterface_SetMotionQuality(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: JPC_MotionQuality);
    pub fn JPC_BodyInterface_GetMotionQuality(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> JPC_MotionQuality;
    pub fn JPC_BodyInterface_GetInverseInertia(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID, arg2: *mut JPC_Mat44);
    pub fn JPC_BodyInterface_SetRestitution(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: f32);
    pub fn JPC_BodyInterface_GetRestitution(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> f32;
    pub fn JPC_BodyInterface_SetFriction(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: f32);
    pub fn JPC_BodyInterface_GetFriction(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> f32;
    pub fn JPC_BodyInterface_SetGravityFactor(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: f32);
    pub fn JPC_BodyInterface_GetGravityFactor(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> f32;
    pub fn JPC_BodyInterface_SetUseManifoldReduction(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID, arg2: bool);
    pub fn JPC_BodyInterface_GetUseManifoldReduction(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> bool;
    pub fn JPC_BodyInterface_GetUserData(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID) -> u64;
    pub fn JPC_BodyInterface_SetUserData(arg0: *const JPC_BodyInterface, arg1: JPC_BodyID, arg2: u64);
    pub fn JPC_BodyInterface_InvalidateContactCache(arg0: *mut JPC_BodyInterface, arg1: JPC_BodyID);
    pub fn JPC_NarrowPhaseQuery_CastRay(arg0: *const JPC_NarrowPhaseQuery, arg1: *mut JPC_NarrowPhaseQuery_CastRayArgs) -> bool;
    pub fn JPC_PhysicsSystem_new() -> *mut JPC_PhysicsSystem;
    pub fn JPC_PhysicsSystem_delete(arg0: *mut JPC_PhysicsSystem);
    pub fn JPC_PhysicsSystem_Init(arg0: *mut JPC_PhysicsSystem, arg1: c_uint, arg2: c_uint, arg3: c_uint, arg4: c_uint, arg5: *mut JPC_BroadPhaseLayerInterface, arg6: *mut JPC_ObjectVsBroadPhaseLayerFilter, arg7: *mut JPC_ObjectLayerPairFilter);
    pub fn JPC_PhysicsSystem_OptimizeBroadPhase(arg0: *mut JPC_PhysicsSystem);
    pub fn JPC_PhysicsSystem_Update(arg0: *mut JPC_PhysicsSystem, arg1: f32, arg2: c_int, arg3: *mut JPC_TempAllocatorImpl, arg4: *mut JPC_JobSystemThreadPool) -> JPC_PhysicsUpdateError;
    pub fn JPC_PhysicsSystem_GetBodyInterface(arg0: *mut JPC_PhysicsSystem) -> *mut JPC_BodyInterface;
    pub fn JPC_PhysicsSystem_SetContactListener(
        arg0: *mut JPC_PhysicsSystem,
        arg1: *mut JPC_ContactListener,
    );
    pub fn JPC_PhysicsSystem_GetBodySurfaceNormal(
        arg0: *const JPC_PhysicsSystem,
        arg1: JPC_BodyID,
        arg2: JPC_SubShapeID,
        arg3: JPC_RVec3,
        arg4: *mut JPC_Vec3,
    ) -> bool;
    pub fn JPC_PhysicsSystem_GetNarrowPhaseQuery(arg0: *const JPC_PhysicsSystem) -> *const JPC_NarrowPhaseQuery;
    pub fn JPC_PhysicsSystem_DrawBodies(arg0: *mut JPC_PhysicsSystem, arg1: *mut JPC_BodyManager_DrawSettings, arg2: *mut JPC_DebugRendererSimple, arg3: *const c_void);
}
