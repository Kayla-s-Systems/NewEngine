fn reverse_northstar_triangle_winding(indices: &mut [u32]) {
    for triangle in indices.as_chunks_mut::<3>().0 {
        triangle.swap(1, 2);
    }
}

fn recalculate_normals(positions: &[[f32; 4]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let a = vec3(positions[triangle[0] as usize]);
        let b = vec3(positions[triangle[1] as usize]);
        let c = vec3(positions[triangle[2] as usize]);
        let face = (b - a).cross(c - a);
        if face.length_squared() > 1.0e-18 && face.is_finite() {
            normals[triangle[0] as usize] += face;
            normals[triangle[1] as usize] += face;
            normals[triangle[2] as usize] += face;
        }
    }
    normals
        .into_iter()
        .map(|normal| {
            let normal = normal.normalize_or_zero();
            if normal.length_squared() <= 1.0e-12 {
                [0.0, 1.0, 0.0]
            } else {
                [normal.x, normal.y, normal.z]
            }
        })
        .collect()
}

#[inline]
fn vec3(value: [f32; 4]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

struct LsbBitReader<'a> {
    bytes: &'a [u8],
    bit: usize,
}

impl<'a> LsbBitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit: 0 }
    }

    fn read(&mut self, count: usize) -> Result<u32, String> {
        if count == 0 {
            return Ok(0);
        }
        let end = self
            .bit
            .checked_add(count)
            .ok_or("quantized bit offset overflow")?;
        if end > self.bytes.len().saturating_mul(8) {
            return Err("quantized stream ended before declared vertices".to_owned());
        }
        let mut value = 0u32;
        for output_bit in 0..count {
            let source = self.bit + output_bit;
            let bit = (self.bytes[source / 8] >> (source % 8)) & 1;
            value |= u32::from(bit) << output_bit;
        }
        self.bit = end;
        Ok(value)
    }
}
