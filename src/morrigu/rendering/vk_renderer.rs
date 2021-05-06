use std::sync::Arc;
use vulkano::{
    device::{Device, DeviceExtensions},
    instance::{
        ApplicationInfo, Instance, PhysicalDevice, PhysicalDeviceType, PhysicalDevicesIter, Version,
    },
    swapchain::Surface,
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

pub struct Renderer {
    instance: Option<Arc<Instance>>,
    surface: Option<Arc<Surface<Window>>>,
    device: Option<Arc<Device>>,
    queue: Option<Arc<vulkano::device::Queue>>,
}

impl Renderer {
    pub fn new() -> Renderer {
        Renderer {
            instance: None,
            surface: None,
            device: None,
            queue: None,
        }
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
