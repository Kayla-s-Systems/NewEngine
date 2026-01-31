use thiserror::Error;

#[derive(Debug, Error)]
pub enum VkRenderError {
    #[error("Missing WindowResource in Resources")]
    MissingWindow,

    #[error("Vulkan error: {0}")]
    Vk(#[from] ash::vk::Result),

    #[error("ash-window error: {0}")]
    AshWindow(String),
}

pub type VkResult<T> = Result<T, VkRenderError>;
