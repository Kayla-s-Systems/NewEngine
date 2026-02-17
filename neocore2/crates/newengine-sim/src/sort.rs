use crate::SystemEntry;

#[inline]
pub fn sort_stage(stage: &mut Vec<SystemEntry>) {
    stage.sort_by(|a, b| {
        a.order
            .cmp(&b.order)
            .then(a.seq.cmp(&b.seq))
    });
}