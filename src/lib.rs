pub mod allocated_types;
pub mod application;
pub mod error;
pub mod material;
pub mod mesh;
pub mod renderer;
pub mod sample_vertex;
pub mod shader;
pub mod texture;
pub mod utils;
pub mod vector_type;

pub mod components;
pub mod ecs_manager;
pub mod systems;

#[cfg(feature = "egui")]
pub mod egui;

mod pipeline_builder;
