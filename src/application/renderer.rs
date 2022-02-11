use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{Surface, Swapchain},
    },
    vk::{self, PhysicalDeviceType},
    Entry, Instance,
};
use std::ffi::{CStr, CString};
use winit::window::Window;

#[cfg(debug_assertions)]
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> u32 {
    let callback_data_deref = *callback_data;
    let message_id_str = (callback_data_deref.message_id_number as i32).to_string();
    let message = if callback_data_deref.p_message.is_null() {
        std::borrow::Cow::from("")
    } else {
        CStr::from_ptr(callback_data_deref.p_message).to_string_lossy()
    };

    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => {
            log::debug!("{message_severity:?} ({message_type:?}): [ID: {message_id_str}] {message}")
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => {
            log::info!("{message_severity:?} ({message_type:?}): [ID: {message_id_str}] {message}")
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => {
            log::warn!("{message_severity:?} ({message_type:?}): [ID: {message_id_str}] {message}")
        }
        _ => {
            log::error!("{message_severity:?} ({message_type:?}): [ID: {message_id_str}] {message}")
        }
    }

    vk::FALSE
}

fn vendor_id_to_str(vendor_id: u32) -> &'static str {
    match vendor_id {
        0x1002 => "AMD",
        0x1010 => "ImgTec",
        0x10DE => "NVIDIA",
        0x13B5 => "ARM",
        0x5143 => "Qualcomm",
        0x8086 => "Intel",
        _ => "unknown",
    }
}

fn device_type_to_str(device_type: PhysicalDeviceType) -> &'static str {
    match device_type {
        PhysicalDeviceType::INTEGRATED_GPU => "integrated GPU",
        PhysicalDeviceType::DISCRETE_GPU => "discrete GPU",
        PhysicalDeviceType::VIRTUAL_GPU => "virtual GPU",
        PhysicalDeviceType::CPU => "CPU",
        _ => "other",
    }
}

struct QueueInfo {
    handle: vk::Queue,
    family_index: u32,
}

struct SurfaceInfo {
    handle: vk::SurfaceKHR,
    format: vk::SurfaceFormatKHR,
    capabilities: vk::SurfaceCapabilitiesKHR,
    loader: Surface,
}

struct SwapchainInfo {
    handle: vk::SwapchainKHR,
    loader: Swapchain,
}

#[allow(dead_code)]
struct DebugMessengerInfo {
    handle: vk::DebugUtilsMessengerEXT,
    loader: DebugUtils,
}

pub struct Renderer {
    entry: ash::Entry,
    instance: ash::Instance,
    surface: SurfaceInfo,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    present_queue: QueueInfo,
    swapchain: SwapchainInfo,
    command_pool: vk::CommandPool,
    primary_command_buffer: vk::CommandBuffer,

    #[allow(dead_code)]
    debug_messenger: Option<DebugMessengerInfo>,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device_wait_idle()
                .expect("Failed to wait for device!");
            self.device.destroy_command_pool(self.command_pool, None);
            self.swapchain
                .loader
                .destroy_swapchain(self.swapchain.handle, None);
            self.device.destroy_device(None);
            self.surface
                .loader
                .destroy_surface(self.surface.handle, None);

            if let Some(debug_messenger) = &self.debug_messenger {
                debug_messenger
                    .loader
                    .destroy_debug_utils_messenger(debug_messenger.handle, None);
            }

            self.instance.destroy_instance(None);
        }
    }
}

pub struct RendererBuilder<'a> {
    window_handle: &'a Window,
    width: u32,
    height: u32,
    preferred_present_mode: vk::PresentModeKHR,
    application_name: CString,
    application_version: u32,
}

impl<'a> RendererBuilder<'a> {
    fn create_instance(&self, entry: &ash::Entry) -> Instance {
        let engine_name = CString::new("Morrigu").unwrap();
        let app_info = vk::ApplicationInfo::builder()
            .application_name(self.application_name.as_c_str())
            .application_version(self.application_version)
            .engine_name(engine_name.as_c_str())
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::make_api_version(0, 1, 2, 0));

        let required_extensions = ash_window::enumerate_required_extensions(self.window_handle)
            .expect("Failed to query extensions!");
        #[allow(unused_mut)]
        let mut raw_required_extensions = required_extensions
            .iter()
            .map(|extension| extension.as_ptr())
            .collect::<Vec<_>>();

        #[allow(unused_assignments)]
        #[allow(unused_mut)]
        let mut raw_layer_names = vec![];
        #[cfg(debug_assertions)]
        {
            let layer_names =
                [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];
            raw_layer_names = layer_names.iter().map(|layer| layer.as_ptr()).collect();

            raw_required_extensions.push(ash::extensions::ext::DebugUtils::name().as_ptr());
        }

        let instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_layer_names(&raw_layer_names)
            .enabled_extension_names(&raw_required_extensions)
            .build();
        unsafe {
            entry
                .create_instance(&instance_info, None)
                .expect("Failed to create Vulkan instance!")
        }
    }

    fn create_debug_messenger(
        &self,
        _entry: &ash::Entry,
        _instance: &ash::Instance,
    ) -> Option<DebugMessengerInfo> {
        #[allow(unused_assignments)]
        #[allow(unused_mut)]
        let mut debug_messenger = None;
        #[cfg(debug_assertions)]
        {
            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
                )
                .pfn_user_callback(Some(vulkan_debug_callback))
                .build();

            let debug_messenger_loader = DebugUtils::new(_entry, _instance);
            let debug_messenger_handle =
                unsafe { debug_messenger_loader.create_debug_utils_messenger(&debug_info, None) }
                    .expect(
                        "Failed to create debug messenger. Try compiling a release build instead?",
                    );

            debug_messenger = Some(DebugMessengerInfo {
                handle: debug_messenger_handle,
                loader: debug_messenger_loader,
            });
        }

        debug_messenger
    }

    fn select_physical_device(
        &self,
        surface: vk::SurfaceKHR,
        instance: &ash::Instance,
        surface_wrapper: &Surface,
        required_version: u32,
    ) -> (vk::PhysicalDevice, u32) {
        let physical_devices = unsafe { instance.enumerate_physical_devices() }
            .expect("Failed to query physical devices!");

        let device_selector =
            |physical_device: &vk::PhysicalDevice| -> Option<(vk::PhysicalDevice, u32)> {
                let raw_physical_device = *physical_device;
                let device_discriminator = |(queue_index, device_queue_info): (
                    usize,
                    &vk::QueueFamilyProperties,
                )|
                 -> Option<(vk::PhysicalDevice, u32)> {
                    let supports_required_version =
                        unsafe { instance.get_physical_device_properties(raw_physical_device) }
                            .api_version
                            >= required_version;
                    let supports_graphics = device_queue_info
                        .queue_flags
                        .contains(vk::QueueFlags::GRAPHICS);
                    let is_compatible_with_surface = unsafe {
                        surface_wrapper.get_physical_device_surface_support(
                            raw_physical_device,
                            queue_index as u32,
                            surface,
                        )
                    }
                    .expect("Failed to query surface compatibility!");

                    if supports_required_version && supports_graphics && is_compatible_with_surface
                    {
                        Some((raw_physical_device, queue_index as u32))
                    } else {
                        None
                    }
                };
                unsafe { instance.get_physical_device_queue_family_properties(raw_physical_device) }
                    .iter()
                    .enumerate()
                    .find_map(device_discriminator)
            };

        physical_devices
            .iter()
            .map(device_selector)
            .flatten()
            .next()
            .expect("Unable to find a suitable physical device!")
    }

    fn create_device(
        &self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> ash::Device {
        let raw_extensions_name = [Swapchain::name().as_ptr()];
        let features = vk::PhysicalDeviceFeatures::default();
        let priorities = [1.0];

        let queue_infos = [vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priorities)
            .build()];

        let device_create_info = vk::DeviceCreateInfo::builder()
            .enabled_features(&features)
            .enabled_extension_names(&raw_extensions_name)
            .queue_create_infos(&queue_infos)
            .build();

        unsafe { instance.create_device(physical_device, &device_create_info, None) }
            .expect("Failed to create logial device!")
    }

    fn select_surface_format(
        &self,
        surface_formats: Vec<vk::SurfaceFormatKHR>,
    ) -> vk::SurfaceFormatKHR {
        surface_formats
            .iter()
            .cloned()
            .find(|&surface_format| {
                surface_format.format == vk::Format::B8G8R8A8_SRGB
                    && surface_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(surface_formats[0])
    }

    fn create_swapchain(
        &self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        surface: &SurfaceInfo,
        surface_wrapper: &Surface,
    ) -> SwapchainInfo {
        let mut requested_image_count = surface.capabilities.min_image_count + 1;
        if surface.capabilities.max_image_count > 0
            && requested_image_count > surface.capabilities.max_image_count
        {
            requested_image_count = surface.capabilities.max_image_count;
        }

        let surface_extent = match surface.capabilities.current_extent.width {
            std::u32::MAX => vk::Extent2D {
                width: self.width,
                height: self.height,
            },
            _ => surface.capabilities.current_extent,
        };

        let present_modes = unsafe {
            surface_wrapper
                .get_physical_device_surface_present_modes(physical_device, surface.handle)
        }
        .expect("Failed to query surface present modes!");
        let present_mode = present_modes
            .iter()
            .cloned()
            .find(|&mode| mode == self.preferred_present_mode)
            .unwrap_or(vk::PresentModeKHR::FIFO);

        let swapchain_wrapper = Swapchain::new(instance, device);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface.handle)
            .min_image_count(requested_image_count)
            .image_color_space(surface.format.color_space)
            .image_format(surface.format.format)
            .image_extent(surface_extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface.capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .image_array_layers(1)
            .build();

        let swapchain = unsafe { swapchain_wrapper.create_swapchain(&swapchain_create_info, None) }
            .expect("Failed to create swapchain!");

        SwapchainInfo {
            handle: swapchain,
            loader: swapchain_wrapper,
        }
    }
}

impl<'a> RendererBuilder<'a> {
    pub fn new(window_handle: &'a Window) -> Self {
        RendererBuilder {
            window_handle,
            width: 1280,
            height: 720,
            preferred_present_mode: vk::PresentModeKHR::MAILBOX,
            application_name: CString::new("").unwrap(),
            application_version: 0,
        }
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_preferred_present_mode(mut self, present_mode: vk::PresentModeKHR) -> Self {
        self.preferred_present_mode = present_mode;
        self
    }

    pub fn with_name(mut self, name: &'a str) -> Self {
        self.application_name = CString::new(name).expect("Invalid application name!");
        self
    }

    pub fn with_version(mut self, major: u32, minor: u32, patch: u32) -> Self {
        self.application_version = vk::make_api_version(0, major, minor, patch);
        self
    }

    pub fn build(self) -> Renderer {
        let entry = Entry::linked();
        let instance = self.create_instance(&entry);
        let debug_messenger = self.create_debug_messenger(&entry, &instance);

        let surface_handle = unsafe {
            ash_window::create_surface(&entry, &instance, &self.window_handle, None)
                .expect("Failed to create rendering surface!")
        };
        let surface_loader = Surface::new(&entry, &instance);

        let required_api_version = (1, 0, 0);
        let (physical_device, queue_family_index) = self.select_physical_device(
            surface_handle,
            &instance,
            &surface_loader,
            vk::make_api_version(
                0,
                required_api_version.0,
                required_api_version.1,
                required_api_version.2,
            ),
        );
        let surface_format = self.select_surface_format(
            unsafe {
                surface_loader.get_physical_device_surface_formats(physical_device, surface_handle)
            }
            .expect("Failed to query physical device formats!"),
        );
        let surface_capabilities = unsafe {
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface_handle)
        }
        .expect("Failed to query physical device capabilities!");
        let surface = SurfaceInfo {
            handle: surface_handle,
            format: surface_format,
            capabilities: surface_capabilities,
            loader: surface_loader,
        };

        let device_properties = unsafe { instance.get_physical_device_properties(physical_device) };
        let device_name = unsafe { CStr::from_ptr(device_properties.device_name.as_ptr()) }
            .to_str()
            .unwrap_or("Invalid");
        let device_vendor = vendor_id_to_str(device_properties.vendor_id);
        let device_type = device_type_to_str(device_properties.device_type);
        let device_supported_version = device_properties.api_version;
        log::info!("Selected device: {device_name}");
        log::debug!("\tVendor: {device_vendor}");
        log::debug!("\tType: {device_type}");
        log::debug!(
            "\tSupported API version: {}.{}.{} (requested {}.{}.{})",
            vk::api_version_major(device_supported_version),
            vk::api_version_minor(device_supported_version),
            vk::api_version_patch(device_supported_version),
            required_api_version.0,
            required_api_version.1,
            required_api_version.2,
        );

        let device = self.create_device(&instance, physical_device, queue_family_index);
        let present_queue = QueueInfo {
            handle: unsafe { device.get_device_queue(queue_family_index, 0) },
            family_index: queue_family_index,
        };

        let swapchain = self.create_swapchain(
            &instance,
            physical_device,
            &device,
            &surface,
            &surface.loader,
        );

        let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(present_queue.family_index);
        let command_pool = unsafe { device.create_command_pool(&command_pool_create_info, None) }
            .expect("Failed to create renderer command pool!");
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .command_buffer_count(1)
            .level(vk::CommandBufferLevel::PRIMARY)
            .build();
        let primary_command_buffer =
            unsafe { device.allocate_command_buffers(&command_buffer_allocate_info) }
                .expect("Failed to allocate primary command buffer!")[0];

        Renderer {
            entry,
            instance,
            surface,
            physical_device,
            device,
            present_queue,
            swapchain,
            command_pool,
            primary_command_buffer,

            debug_messenger,
        }
    }
}
