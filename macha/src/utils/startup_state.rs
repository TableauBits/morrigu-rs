use std::fmt::Display;

use clap::ValueEnum;
use morrigu::application::{ApplicationState, BuildableApplicationState};

#[derive(PartialEq, Copy, Clone, ValueEnum)]
pub enum SwitchableStates {
    Editor,
    GLTFLoader,
    CSTest,
    PBRTest,

    #[cfg(feature = "ray_tracing")]
    RTTest,
}

impl Display for SwitchableStates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            SwitchableStates::Editor => "Macha Editor",
            SwitchableStates::GLTFLoader => "GLTF Loader and Viewer",
            SwitchableStates::CSTest => "Compute Shader Test",
            SwitchableStates::PBRTest => "PBR Test",

            #[cfg(feature = "ray_tracing")]
            SwitchableStates::RTTest => "Ray Tracing Test",
        };

        write!(f, "{}", name)
    }
}

pub struct StartupState {
    desired_state: SwitchableStates,
}

impl BuildableApplicationState<SwitchableStates> for StartupState {
    fn build(_context: &mut morrigu::application::StateContext, data: SwitchableStates) -> Self {
        Self {
            desired_state: data,
        }
    }
}

impl ApplicationState for StartupState {
    fn flow<'flow>(
        &mut self,
        context: &mut morrigu::application::StateContext,
    ) -> morrigu::application::StateFlow<'flow> {
        match self.desired_state {
            SwitchableStates::Editor => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::editor::MachaState::build(context, ()),
            )),
            SwitchableStates::GLTFLoader => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::gltf_loader::GLTFViewerState::build(context, ()),
            )),
            SwitchableStates::CSTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::compute_shader_test::CSTState::build(context, ()),
            )),
            SwitchableStates::PBRTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::pbr_test::PBRState::build(context, ()),
            )),

            #[cfg(feature = "ray_tracing")]
            SwitchableStates::RTTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::rt_test::RayTracerState::build(context, ()),
            )),
        }
    }
}
