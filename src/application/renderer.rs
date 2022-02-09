use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{Surface, Swapchain},
    },
    vk::{self, PhysicalDeviceType},
    Entry, Instance,
};
use log::{debug, error, info, warn};
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
            debug!("{message_severity:?} ({message_type:?}): [ID: {message_id_str}] {message}")
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => {
            info!("{message_severity:?} ({message_type:?}): [ID: {message_id_str}] {message}")
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => {
            warn!("{message_severity:?} ({message_type:?}): [ID: {message_id_str}] {message}")
        }
        _ => {
            error!("{message_severity:?} ({message_type:?}): [ID: {message_id_str}] {message}")
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

pub struct Renderer {
    #[allow(dead_code)]
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,

    present_queue: QueueInfo,
    device: ash::Device,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    instance: ash::Instance,
    entry: ash::Entry,
}

pub struct RendererBuilder<'a> {
    window_handle: &'a Window,
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
        let mut raw_required_extensions = required_extensions
            .iter()
            .map(|extension| extension.as_ptr())
            .collect::<Vec<_>>();

        #[allow(unused_assignments)]
        let mut raw_layer_names = vec![];
        #[cfg(debug_assertions)]
        {
            let layer_names =
                [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];
            raw_layer_names = layer_names.iter().map(|layer| layer.as_ptr()).collect();

            raw_required_extensions.push(DebugUtils::name().as_ptr());
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
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> Option<vk::DebugUtilsMessengerEXT> {
        #[allow(unused_assignments)]
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

            debug_messenger = unsafe {
                Some(DebugUtils::new(entry, instance)
                .create_debug_utils_messenger(&debug_info, None)
                .expect("Failed to create debug messenger. Try compiling a release build instead?")
					)
            };
        }

        debug_messenger
    }

    fn select_physical_device(
        &self,
        entry: &ash::Entry,
        instance: &ash::Instance,
        surface: vk::SurfaceKHR,
        required_version: u32,
    ) -> (vk::PhysicalDevice, u32) {
        let physical_devices = unsafe { instance.enumerate_physical_devices() }
            .expect("Failed to query physical devices!");

        let surface_wrapper = Surface::new(entry, instance);

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
}

impl<'a> RendererBuilder<'a> {
    pub fn new(window_handle: &'a Window) -> Self {
        RendererBuilder {
            window_handle,
            application_name: CString::new("").unwrap(),
            application_version: 0,
        }
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

        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, &self.window_handle, None)
                .expect("Failed to create rendering surface!")
        };

        let required_api_version = (1, 0, 0);
        let (physical_device, queue_family_index) = self.select_physical_device(
            &entry,
            &instance,
            surface,
            vk::make_api_version(
                0,
                required_api_version.0,
                required_api_version.1,
                required_api_version.2,
            ),
        );

        let device_properties = unsafe { instance.get_physical_device_properties(physical_device) };
        let device_name = unsafe { CStr::from_ptr(device_properties.device_name.as_ptr()) }
            .to_str()
            .unwrap_or("Invalid");
        let device_vendor = vendor_id_to_str(device_properties.vendor_id);
        let device_type = device_type_to_str(device_properties.device_type);
        let device_supported_version = device_properties.api_version;
        info!("Selected device: {device_name}");
        debug!("\tVendor: {device_vendor}");
        debug!("\tType: {device_type}");
        debug!(
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

        Renderer {
            debug_messenger,
            present_queue,
            device,
            physical_device,
            surface,
            instance,
            entry,
        }
    }
}
