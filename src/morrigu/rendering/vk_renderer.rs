use std::sync::Arc;
use vulkano::{
    device::{Device, DeviceExtensions},
    format::Format,
    image::{ImageUsage, SwapchainImage},
    instance::{
        debug::{DebugCallback, MessageSeverity, MessageType},
        ApplicationInfo, Instance, PhysicalDevice, PhysicalDeviceType, PhysicalDevicesIter,
        Version,
    },
    swapchain::{
        Capabilities, ColorSpace, FullscreenExclusive, PresentMode, Surface, SurfaceTransform,
        Swapchain,
    },
};
use vulkano_win::{self, VkSurfaceBuild};
use winit::{
    dpi::PhysicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub struct WindowSpecification<'a> {
    pub name: &'a str,
    pub version: (u16, u16, u16),
    pub width: u32,
    pub height: u32,
}

#[derive(Default)]
pub struct Renderer {
    instance: Option<Arc<Instance>>,
    surface: Option<Arc<Surface<Window>>>,
    device: Option<Arc<Device>>,
    capabilities: Option<Capabilities>,
    queue: Option<Arc<vulkano::device::Queue>>,
    swapchain: Option<Arc<Swapchain<Window>>>,
    images: Vec<Arc<SwapchainImage<Window>>>,
    sc_format: Option<Format>,

    _debug_callback: Option<DebugCallback>,
}

impl Renderer {
    pub fn new() -> Renderer {
        Default::default()
    }

    pub fn init(&mut self, window: &WindowSpecification, event_loop: &EventLoop<()>) {
        self.init_vulkan(
            window.name,
            Version {
                major: window.version.0,
                minor: window.version.1,
                patch: window.version.2,
            },
            window,
            event_loop,
        );

        self.init_swapchain();
    }

    fn init_vulkan(
        &mut self,
        app_name: &str,
        app_version: Version,
        window: &WindowSpecification,
        event_loop: &EventLoop<()>,
    ) {
        self.init_instance(app_name, app_version);
        self.init_device(window, event_loop);
    }

    fn init_instance(&mut self, app_name: &str, app_version: Version) {
        let version = vulkano::instance::Version {
            major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
            patch: env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
        };
        let app_info = ApplicationInfo {
            application_name: Some(std::borrow::Cow::Owned(String::from(app_name))),
            application_version: Some(app_version),
            engine_name: Some(std::borrow::Cow::Owned(String::from("Morrigu"))),
            engine_version: Some(version),
        };
        let required_extensions = vulkano_win::required_extensions();
        self.instance = Some(
            Instance::new(
                Some(&app_info),
                &required_extensions,
                vec!["VK_LAYER_KHRONOS_validation"],
            )
            .expect("Failed to create Vulkan instance!"),
        );

        self._debug_callback = DebugCallback::new(
            self.instance.as_ref().unwrap(),
            MessageSeverity {
                error: true,
                warning: true,
                information: true,
                verbose: true,
            },
            MessageType {
                general: true,
                validation: true,
                performance: true,
            },
            |msg| {
                println!("[VK]: {}", msg.description);
            },
        )
        .ok();
    }

    fn init_device(&mut self, window: &WindowSpecification, event_loop: &EventLoop<()>) {
        // physical device selection
        let physical =
            select_device(PhysicalDevice::enumerate(self.instance.as_ref().unwrap())).unwrap();

        // surface creation
        self.surface = Some(
            WindowBuilder::new()
                .with_title(window.name)
                .with_inner_size(PhysicalSize {
                    width: window.width,
                    height: window.height,
                })
                .build_vk_surface(&event_loop, self.instance.clone().unwrap())
                .expect("Failed to create window!"),
        );

        self.capabilities = self.surface.as_ref().unwrap().capabilities(physical).ok();

        // find a queue family that supports graphics and works with our surface
        let queue_family = physical
            .queue_families()
            .find(|&queue| {
                queue.supports_graphics()
                    && self
                        .surface
                        .as_ref()
                        .unwrap()
                        .is_supported(queue)
                        .unwrap_or(false)
            })
            .expect("No valid family queue!");

        // logical device creation
        let device_extentions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };
        let (device, mut queues) = Device::new(
            physical,
            physical.supported_features(),
            &device_extentions,
            [(queue_family, 1.0)].iter().cloned(),
        )
        .expect("Failed to create logical device!");
        self.device = Some(device);
        self.queue = Some(queues.next().expect("Device Queue list cannot be empty!"));
    }

    fn init_swapchain(&mut self) {
        let supported_alpha = self
            .capabilities
            .as_ref()
            .unwrap()
            .supported_composite_alpha
            .iter()
            .next()
            .unwrap();

        self.sc_format = Some(self.capabilities.as_ref().unwrap().supported_formats[0].0);

        let dims: [u32; 2] = self.surface.as_ref().unwrap().window().inner_size().into();

        let (swapchain, images) = Swapchain::new(
            self.device.as_ref().unwrap().clone(),
            self.surface.as_ref().unwrap().clone(),
            self.capabilities.as_ref().unwrap().min_image_count,
            self.sc_format.unwrap(),
            dims,
            1,
            ImageUsage::color_attachment(),
            self.queue.as_ref().unwrap(),
            SurfaceTransform::Identity,
            supported_alpha,
            PresentMode::Fifo,
            FullscreenExclusive::Default,
            true,
            ColorSpace::SrgbNonLinear,
        )
        .expect("Failed to create swapchain!");

        self.swapchain = Some(swapchain);
        self.images = images;
    }
}

fn is_candidate_suitable(canditate: &PhysicalDevice) -> bool {
    let supported_features = canditate.supported_features();
    let mut queue_families = canditate.queue_families();

    if !supported_features.independent_blend {
        return false;
    }
    if !supported_features.sampler_anisotropy {
        return false;
    }
    if !queue_families.any(|queue| queue.supports_graphics()) {
        return false;
    }
    true
}

fn select_device(canditates: PhysicalDevicesIter) -> Option<PhysicalDevice> {
    let mut best_candidate = None;
    for canditate in canditates.filter(|device| is_candidate_suitable(device)) {
        if canditate.ty() == PhysicalDeviceType::DiscreteGpu {
            best_candidate = Some(canditate);
        }
    }
    match best_candidate {
        None => {
            println!("No valid physical device found!")
        }
        Some(device) => {
            println!("Selected the following physical device: {}", device.name());
        }
    }
    best_candidate
}
