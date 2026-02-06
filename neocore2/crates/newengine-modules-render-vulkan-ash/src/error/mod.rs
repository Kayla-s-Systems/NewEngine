use thiserror::Error;

pub type VkResult<T> = Result<T, VkRenderError>;

#[derive(Debug, Error)]
pub enum VkRenderError {
    #[error("Missing Winit window handles in Resources")]
    MissingWindowHandles,

    #[error("Missing initial window size in Resources")]
    MissingWindowSize,

    #[error("{0}")]
    AshWindow(String),

    #[error("Invalid render state: {0}")]
    InvalidState(&'static str),

    #[error("Vulkan error: {0}")]
    Vk(#[from] ash::vk::Result),
}
