pub mod allocated_types;
pub mod application;
pub mod compute_shader;
pub mod cubemap;
pub mod descriptor_resources;
pub mod material;
pub mod math_types;
pub mod mesh;
pub mod pipeline_barrier;
pub mod renderer;
pub mod shader;
pub mod texture;
pub mod utils;
pub mod vertices;

pub mod components;
pub mod ecs_manager;
pub mod systems;

#[cfg(feature = "egui")]
pub mod egui_integration;

mod pipeline_builder;

// Core re-exports
pub use ash;
pub use bevy_ecs;
pub use winit;

#[cfg(feature = "egui")]
pub use egui;
