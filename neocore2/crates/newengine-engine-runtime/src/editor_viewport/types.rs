use newengine_ecs::EntityId;
use newengine_math::Vec3;
use newengine_transform::Transform;

pub(super) const EDITOR_HISTORY_LIMIT: usize = 256;
pub(super) const GIZMO_HANDLE_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorGizmoAxis {
    X,
    Y,
    Z,
}

impl EditorGizmoAxis {
    #[inline]
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    #[inline]
    pub(crate) const fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    #[inline]
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    #[inline]
    pub(super) const fn color(self) -> [f32; 4] {
        match self {
            Self::X => [0.92, 0.16, 0.14, 1.0],
            Self::Y => [0.18, 0.78, 0.28, 1.0],
            Self::Z => [0.18, 0.42, 0.96, 1.0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorGizmoPlane {
    XY,
    XZ,
    YZ,
}

impl EditorGizmoPlane {
    #[inline]
    pub(super) const fn basis(self) -> (Vec3, Vec3) {
        match self {
            Self::XY => (Vec3::X, Vec3::Y),
            Self::XZ => (Vec3::X, Vec3::Z),
            Self::YZ => (Vec3::Y, Vec3::Z),
        }
    }

    #[inline]
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::XY => "XY",
            Self::XZ => "XZ",
            Self::YZ => "YZ",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorGizmoHandle {
    Axis(EditorGizmoAxis),
    Plane(EditorGizmoPlane),
    Center,
}

impl EditorGizmoHandle {
    #[inline]
    pub(super) const fn index(self) -> usize {
        match self {
            Self::Axis(axis) => axis.index(),
            Self::Plane(EditorGizmoPlane::XY) => 3,
            Self::Plane(EditorGizmoPlane::XZ) => 4,
            Self::Plane(EditorGizmoPlane::YZ) => 5,
            Self::Center => 6,
        }
    }

    #[inline]
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Axis(axis) => axis.name(),
            Self::Plane(plane) => plane.name(),
            Self::Center => "Center",
        }
    }

    #[inline]
    pub(super) const fn color(self) -> [f32; 4] {
        match self {
            Self::Axis(axis) => axis.color(),
            Self::Plane(EditorGizmoPlane::XY) => [0.88, 0.78, 0.18, 0.92],
            Self::Plane(EditorGizmoPlane::XZ) => [0.78, 0.22, 0.82, 0.92],
            Self::Plane(EditorGizmoPlane::YZ) => [0.16, 0.76, 0.78, 0.92],
            Self::Center => [0.92, 0.92, 0.92, 1.0],
        }
    }
}

/// Runtime-only component attached to editor gizmo geometry.
/// It deliberately never becomes authored scene data.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EditorGizmoAxisComponent {
    pub(crate) handle: EditorGizmoHandle,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TransformTransaction {
    pub(super) entity: EntityId,
    pub(super) before: Transform,
    pub(super) after: Transform,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ActiveTransformDrag {
    pub(super) entity: EntityId,
    pub(super) handle: EditorGizmoHandle,
    pub(super) axis_vector: Vec3,
    pub(super) plane_a: Vec3,
    pub(super) plane_b: Vec3,
    pub(super) before: Transform,
    pub(super) accumulated: f32,
    pub(super) accumulated_world: Vec3,
}
