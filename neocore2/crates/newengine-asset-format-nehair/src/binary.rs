use newengine_render_api::{
    HairCollisionCapsuleV1, HairGroomAssetV1, HairGroomRef, HairGuidePointV1, HairGuideStrandV1,
};

pub const NEHAIR_MAGIC: [u8; 8] = *b"NEHAIR\0\0";
pub const NEHAIR_VERSION_V1: u16 = 1;
const HEADER_BYTES: usize = 32;
const MAX_PAYLOAD_BYTES: usize = 256 * 1024 * 1024;

pub fn encode_nehair_v1(asset: &HairGroomAssetV1) -> Result<Vec<u8>, String> {
    let asset = asset.clone().normalized()?;
    let mut payload = Writer::default();
    payload.string(asset.groom.as_str())?;
    payload.u32(
        asset
            .guide_points
            .len()
            .try_into()
            .map_err(|_| "too many guide points")?,
    );
    for point in &asset.guide_points {
        for value in point.rest_position {
            payload.f32(value);
        }
        payload.f32(point.inverse_mass);
    }
    payload.u32(
        asset
            .guide_strands
            .len()
            .try_into()
            .map_err(|_| "too many guide strands")?,
    );
    for strand in &asset.guide_strands {
        payload.u32(strand.first_point);
        payload.u16(strand.point_count);
        payload.u16(strand.group);
        payload.f32(strand.root_uv[0]);
        payload.f32(strand.root_uv[1]);
        payload.u16(strand.root_joint_index);
        payload.u16(0);
    }
    payload.u32(
        asset
            .collision_capsules
            .len()
            .try_into()
            .map_err(|_| "too many collision capsules")?,
    );
    for capsule in &asset.collision_capsules {
        payload.u16(capsule.joint_index);
        payload.u16(0);
        payload.f32(capsule.radius);
        for value in capsule.local_a {
            payload.f32(value);
        }
        for value in capsule.local_b {
            payload.f32(value);
        }
    }
    payload.u8(asset.follow_strands_per_guide);
    payload.bytes.extend_from_slice(&[0; 3]);

    if payload.bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "NEHAIR payload {} exceeds safety limit {}",
            payload.bytes.len(),
            MAX_PAYLOAD_BYTES
        ));
    }
    let digest = blake3::hash(&payload.bytes);
    let mut output = Vec::with_capacity(HEADER_BYTES + payload.bytes.len());
    output.extend_from_slice(&NEHAIR_MAGIC);
    output.extend_from_slice(&NEHAIR_VERSION_V1.to_le_bytes());
    output.extend_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
    output.extend_from_slice(&(payload.bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(&digest.as_bytes()[..16]);
    debug_assert_eq!(output.len(), HEADER_BYTES);
    output.extend_from_slice(&payload.bytes);
    Ok(output)
}

pub fn decode_nehair(bytes: &[u8]) -> Result<HairGroomAssetV1, String> {
    if bytes.len() < HEADER_BYTES {
        return Err("NEHAIR container shorter than header".to_owned());
    }
    if bytes[..8] != NEHAIR_MAGIC {
        return Err("NEHAIR magic mismatch".to_owned());
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != NEHAIR_VERSION_V1 {
        return Err(format!("unsupported NEHAIR version {version}"));
    }
    let header_bytes = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    if header_bytes != HEADER_BYTES {
        return Err(format!("invalid NEHAIR header size {header_bytes}"));
    }
    let payload_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    if payload_len > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "NEHAIR payload length {payload_len} exceeds safety limit"
        ));
    }
    let end = header_bytes
        .checked_add(payload_len)
        .ok_or_else(|| "NEHAIR payload length overflow".to_owned())?;
    if end != bytes.len() {
        return Err(format!(
            "NEHAIR byte length mismatch header={} payload={} file={}",
            header_bytes,
            payload_len,
            bytes.len()
        ));
    }
    let payload = &bytes[header_bytes..end];
    let digest = blake3::hash(payload);
    if digest.as_bytes()[..16] != bytes[16..32] {
        return Err("NEHAIR payload digest mismatch".to_owned());
    }

    let mut reader = Reader::new(payload);
    let groom = HairGroomRef::new(reader.string()?);
    let point_count = reader.count("guide point", 1_048_576)?;
    let mut guide_points = Vec::with_capacity(point_count);
    for _ in 0..point_count {
        guide_points.push(HairGuidePointV1 {
            rest_position: [reader.f32()?, reader.f32()?, reader.f32()?],
            inverse_mass: reader.f32()?,
        });
    }
    let strand_count = reader.count("guide strand", 1_048_576)?;
    let mut guide_strands = Vec::with_capacity(strand_count);
    for _ in 0..strand_count {
        let first_point = reader.u32()?;
        let point_count = reader.u16()?;
        let group = reader.u16()?;
        let root_uv = [reader.f32()?, reader.f32()?];
        let root_joint_index = reader.u16()?;
        let _reserved = reader.u16()?;
        guide_strands.push(HairGuideStrandV1 {
            first_point,
            point_count,
            group,
            root_uv,
            root_joint_index,
        });
    }
    let capsule_count = reader.count("collision capsule", 65_536)?;
    let mut collision_capsules = Vec::with_capacity(capsule_count);
    for _ in 0..capsule_count {
        let joint_index = reader.u16()?;
        let _reserved = reader.u16()?;
        let radius = reader.f32()?;
        let local_a = [reader.f32()?, reader.f32()?, reader.f32()?];
        let local_b = [reader.f32()?, reader.f32()?, reader.f32()?];
        collision_capsules.push(HairCollisionCapsuleV1 {
            joint_index,
            radius,
            local_a,
            local_b,
        });
    }
    let follow_strands_per_guide = reader.u8()?;
    reader.skip(3)?;
    reader.finish()?;

    HairGroomAssetV1 {
        groom,
        guide_points,
        guide_strands,
        collision_capsules,
        follow_strands_per_guide,
    }
    .normalized()
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }
    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn string(&mut self, value: &str) -> Result<(), String> {
        let bytes = value.as_bytes();
        let len: u16 = bytes
            .len()
            .try_into()
            .map_err(|_| format!("NEHAIR string too long: {} bytes", bytes.len()))?;
        self.u16(len);
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or_else(|| "NEHAIR cursor overflow".to_owned())?;
        if end > self.bytes.len() {
            return Err(format!(
                "NEHAIR truncated payload at offset {} need {} bytes",
                self.cursor, len
            ));
        }
        let out = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn string(&mut self) -> Result<String, String> {
        let len = self.u16()? as usize;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|error| format!("NEHAIR string is not UTF-8: {error}"))
    }
    fn count(&mut self, label: &str, max: usize) -> Result<usize, String> {
        let count = self.u32()? as usize;
        if count > max {
            return Err(format!("NEHAIR {label} count {count} exceeds {max}"));
        }
        Ok(count)
    }
    fn skip(&mut self, len: usize) -> Result<(), String> {
        let _ = self.take(len)?;
        Ok(())
    }
    fn finish(self) -> Result<(), String> {
        if self.cursor != self.bytes.len() {
            return Err(format!(
                "NEHAIR payload has {} trailing bytes",
                self.bytes.len() - self.cursor
            ));
        }
        Ok(())
    }
}
