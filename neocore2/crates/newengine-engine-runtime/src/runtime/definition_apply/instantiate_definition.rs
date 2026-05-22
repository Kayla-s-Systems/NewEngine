use newengine_math::Mat4;

#[derive(Clone, Debug)]
pub struct InstantiateDefinitionCommand {
    pub definition_ref: String,
    pub transform: Mat4,
}
