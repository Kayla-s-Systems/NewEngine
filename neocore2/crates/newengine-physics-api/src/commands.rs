use serde::{Deserialize, Serialize};

use crate::{PhysicsEntityKey, PhysicsQuat, PhysicsVec3};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsCommandKindDto {
    SetBodyPose {
        entity: PhysicsEntityKey,
        position: PhysicsVec3,
        rotation: PhysicsQuat,
    },
    SetLinearVelocity {
        entity: PhysicsEntityKey,
        velocity: PhysicsVec3,
    },
    DestroyBody {
        entity: PhysicsEntityKey,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsCommandDto {
    pub seq: u64,
    pub kind: PhysicsCommandKindDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsQueryKindDto {
    Ray {
        origin: PhysicsVec3,
        dir: PhysicsVec3,
        max_t: f32,
    },
    Sphere {
        center: PhysicsVec3,
        radius: f32,
    },
    Aabb {
        min: PhysicsVec3,
        max: PhysicsVec3,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsQueryDto {
    pub seq: u64,
    /// Optional entity excluded from query hits. Legacy producers that leave this
    /// unset retain the historical `seq == owner entity` self-exclusion behavior
    /// in first-party backends.
    #[serde(default)]
    pub ignore_entity: Option<PhysicsEntityKey>,
    pub kind: PhysicsQueryKindDto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_query_json_defaults_explicit_ignore_entity_to_none() {
        let json = r#"{
            "seq": 7,
            "kind": {"Ray": {"origin": [0.0,0.0,0.0], "dir": [0.0,0.0,-1.0], "max_t": 12.0}}
        }"#;
        let query: PhysicsQueryDto = serde_json::from_str(json).expect("legacy physics query JSON");
        assert_eq!(query.seq, 7);
        assert_eq!(query.ignore_entity, None);
    }
}
