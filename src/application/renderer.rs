use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{Surface, Swapchain},
    },
    vk::{self, PhysicalDeviceType},
    Entry, Instance,
};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use std::{
    ffi::{CStr, CString},
    mem::size_of,
};
use winit::window::Window;

use nalgebra_glm as glm;

use crate::application::allocated_types::{
    AllocatedBuffer, AllocatedBufferBuilder, AllocatedImage,
};

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

struct TimeData {
    time: glm::Vec4,
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
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    depth_image: AllocatedImage,
    loader: Swapchain,
}

#[allow(dead_code)]
struct DebugMessengerInfo {
    handle: vk::DebugUtilsMessengerEXT,
    loader: DebugUtils,
}

struct SyncObjects {
    render_fence: vk::Fence,
    present_semaphore: vk::Semaphore,
    render_semaphore: vk::Semaphore,
}

struct DescriptorInfo {
    handle: vk::DescriptorSet,
    layout: vk::DescriptorSetLayout,
    buffer: Option<AllocatedBuffer>,
}

pub struct Renderer {
    pub clear_color: [f32; 4],

    width: u32,
    height: u32,
    next_image_index: u32,

    #[allow(dead_code)]
    debug_messenger: Option<DebugMessengerInfo>,

    descriptors: [DescriptorInfo; 2],
    descriptor_pool: vk::DescriptorPool,
    sync_objects: SyncObjects,
    primary_command_buffer: vk::CommandBuffer,
    command_pool: vk::CommandPool,
    swapchain_framebuffers: Vec<vk::Framebuffer>,
    primary_render_pass: vk::RenderPass,
    swapchain: SwapchainInfo,
    present_queue: QueueInfo,
    gpu_allocator: Option<Allocator>,
    device: ash::Device,
    physical_device: vk::PhysicalDevice,
    surface: SurfaceInfo,
    instance: ash::Instance,
    entry: ash::Entry,
}

pub struct RendererBuilder<'a> {
    window_handle: &'a Window,
    application_name: CString,
    application_version: u32,
    width: u32,
    height: u32,
    preferred_present_mode: vk::PresentModeKHR,
    input_attachments: Vec<(vk::AttachmentDescription, vk::AttachmentReference)>,
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
            .expect("Failed to query extensions");
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
                .expect("Failed to create Vulkan instance")
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
        surface_loader: &Surface,
        required_version: u32,
    ) -> (vk::PhysicalDevice, u32) {
        let physical_devices = unsafe { instance.enumerate_physical_devices() }
            .expect("Failed to query physical devices");

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
                        surface_loader.get_physical_device_surface_support(
                            raw_physical_device,
                            queue_index as u32,
                            surface,
                        )
                    }
                    .expect("Failed to query surface compatibility");

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
            .filter_map(device_selector)
            .next()
            .unwrap_or_else(|| {
                panic!(
                    "Unable to find a suitable physical device. Candidates were {:#?}",
                    physical_devices
                        .iter()
                        .map(|physical_device| -> &str {
                            unsafe {
                                CStr::from_ptr(
                                    instance
                                        .get_physical_device_properties(*physical_device)
                                        .device_name
                                        .as_ptr(),
                                )
                                .to_str()
                                .unwrap_or("Invalid name")
                            }
                        })
                        .collect::<Vec<_>>()
                )
            })
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
            .expect("Failed to create logial device")
    }

    fn create_allocator(
        &self,
        instance: ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: ash::Device,
    ) -> Allocator {
        Allocator::new(&AllocatorCreateDesc {
            instance,
            physical_device,
            device,
            debug_settings: Default::default(),
            buffer_device_address: false,
        })
        .expect("Failed to create GPU allocator")
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
        surface_loader: &Surface,
        allocator: &mut Allocator,
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
            surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface.handle)
        }
        .expect("Failed to query surface present modes");
        let present_mode = present_modes
            .iter()
            .cloned()
            .find(|&mode| mode == self.preferred_present_mode)
            .unwrap_or(vk::PresentModeKHR::FIFO);

        let swapchain_loader = Swapchain::new(instance, device);

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

        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None) }
            .expect("Failed to create swapchain");

        let image_view_creator = |&image: &vk::Image| {
            let create_view_info = vk::ImageViewCreateInfo::builder()
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(surface.format.format)
                .components(vk::ComponentMapping {
                    r: vk::ComponentSwizzle::R,
                    g: vk::ComponentSwizzle::G,
                    b: vk::ComponentSwizzle::B,
                    a: vk::ComponentSwizzle::A,
                })
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image(image)
                .build();
            unsafe { device.create_image_view(&create_view_info, None) }
                .expect("Failed to ceate swapchain image views")
        };

        let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain) }
            .expect("Failed to get swapchain images");
        let swapchain_image_views = swapchain_images.iter().map(image_view_creator).collect();

        let depth_extent = vk::Extent3D::builder()
            .width(surface_extent.width)
            .height(surface_extent.height)
            .depth(1)
            .build();
        let depth_image = AllocatedImage::builder(depth_extent)
            .depth_image_default()
            .build(device, allocator)
            .expect("Failed to build depth image");

        SwapchainInfo {
            handle: swapchain,
            images: swapchain_images,
            image_views: swapchain_image_views,
            depth_image,
            loader: swapchain_loader,
        }
    }

    fn create_render_passes(
        &self,
        surface: &SurfaceInfo,
        depth_image: &AllocatedImage,
        device: &ash::Device,
    ) -> vk::RenderPass {
        let color_attachment = vk::AttachmentDescription {
            format: surface.format.format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        };
        let depth_attachment = vk::AttachmentDescription {
            format: depth_image.format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        };

        let color_attachment_ref = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        };
        let depth_attachment_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };

        let input_attachment_ref: Vec<vk::AttachmentReference> = self
            .input_attachments
            .clone()
            .iter()
            .map(|pair| pair.1)
            .collect();

        let subpass_description = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .input_attachments(&input_attachment_ref)
            .color_attachments(&[color_attachment_ref])
            .depth_stencil_attachment(&depth_attachment_ref)
            .build();

        let mut attachment_descriptions = vec![color_attachment, depth_attachment];
        attachment_descriptions.append(
            &mut self
                .input_attachments
                .clone()
                .iter()
                .map(|pair| pair.0)
                .collect::<Vec<vk::AttachmentDescription>>(),
        );

        let renderpass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachment_descriptions)
            .subpasses(&[subpass_description])
            .build();

        unsafe { device.create_render_pass(&renderpass_info, None) }
            .expect("Failed to create render pass")
    }

    fn create_framebuffers(
        &self,
        render_pass: vk::RenderPass,
        swapchain: &SwapchainInfo,
        device: &ash::Device,
    ) -> Vec<vk::Framebuffer> {
        let mut framebuffer_create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass)
            .width(self.width)
            .height(self.height)
            .layers(1)
            .build();
        framebuffer_create_info.attachment_count = 2;

        let mut framebuffers = vec![];
        for swapchain_image_view in swapchain.image_views.clone() {
            framebuffer_create_info.p_attachments =
                [swapchain_image_view, swapchain.depth_image.view].as_ptr();
            framebuffers.push(
                unsafe { device.create_framebuffer(&framebuffer_create_info, None) }
                    .expect("Failed to create framebuffer"),
            );
        }

        framebuffers
    }

    fn create_sync_objects(&self, device: &ash::Device) -> SyncObjects {
        let render_fence = unsafe {
            device.create_fence(
                &vk::FenceCreateInfo {
                    flags: vk::FenceCreateFlags::SIGNALED,
                    ..Default::default()
                },
                None,
            )
        }
        .expect("Failed to create render fence");
        let present_semaphore =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                .expect("Failed to create present semaphore");
        let render_semaphore =
            unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }
                .expect("Failed to create render semaphore");

        SyncObjects {
            present_semaphore,
            render_fence,
            render_semaphore,
        }
    }

    fn create_descriptors(
        &self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> (vk::DescriptorPool, [DescriptorInfo; 2]) {
        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
            .max_sets(2)
            .pool_sizes(&[vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 2,
            }])
            .build();
        let descriptor_pool = unsafe { device.create_descriptor_pool(&descriptor_pool_info, None) }
            .expect("Failed to create descriptor pool");

        let level_0_bindings = [vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            ..Default::default()
        }];
        let level_0_layout_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&level_0_bindings)
            .build();
        let level_0_layout =
            unsafe { device.create_descriptor_set_layout(&level_0_layout_info, None) }
                .expect("Failed to create descriptor set 0 layout");
        let level_0_allocation_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&[level_0_layout])
            .build();
        let level_0_handle = unsafe { device.allocate_descriptor_sets(&level_0_allocation_info) }
            .expect("Failed to allocate level 0 descriptor")[0];
        let time_buffer_size: u64 = size_of::<TimeData>().try_into().unwrap();
        let time_buffer = AllocatedBufferBuilder::uniform_buffer_default(time_buffer_size)
            .build(device, allocator)
            .expect("Failed to create time buffer");
        let time_buffer_info = vk::DescriptorBufferInfo {
            buffer: time_buffer.handle,
            offset: 0,
            range: time_buffer_size,
        };
        let time_set_write = vk::WriteDescriptorSet {
            dst_set: level_0_handle,
            dst_binding: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            p_buffer_info: &time_buffer_info,
            ..Default::default()
        };
        unsafe { device.update_descriptor_sets(&[time_set_write], &[]) };

        let level_1_layout_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&[])
            .build();
        let level_1_layout =
            unsafe { device.create_descriptor_set_layout(&level_1_layout_info, None) }
                .expect("Failed to create descriptor set 0 layout");
        let level_1_allocation_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&[level_1_layout])
            .build();
        let level_1_handle = unsafe { device.allocate_descriptor_sets(&level_1_allocation_info) }
            .expect("Failed to allocate level 1 descriptor")[0];

        (
            descriptor_pool,
            [
                DescriptorInfo {
                    handle: level_0_handle,
                    layout: level_0_layout,
                    buffer: Some(time_buffer),
                },
                DescriptorInfo {
                    handle: level_1_handle,
                    layout: level_1_layout,
                    buffer: None,
                },
            ],
        )
    }
}

impl<'a> RendererBuilder<'a> {
    pub fn new(window_handle: &'a Window) -> Self {
        RendererBuilder {
            window_handle,
            application_name: CString::new("").unwrap(),
            application_version: 0,
            width: 1280,
            height: 720,
            preferred_present_mode: vk::PresentModeKHR::MAILBOX,
            input_attachments: vec![],
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
        self.application_name = CString::new(name).expect("Invalid application name");
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
                .expect("Failed to create rendering surface")
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
            .expect("Failed to query physical device formats"),
        );
        let surface_capabilities = unsafe {
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface_handle)
        }
        .expect("Failed to query physical device capabilities");
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

        let mut gpu_allocator =
            self.create_allocator(instance.clone(), physical_device, device.clone());

        let swapchain = self.create_swapchain(
            &instance,
            physical_device,
            &device,
            &surface,
            &surface.loader,
            &mut gpu_allocator,
        );

        let primary_render_pass =
            self.create_render_passes(&surface, &swapchain.depth_image, &device);

        let swapchain_framebuffers =
            self.create_framebuffers(primary_render_pass, &swapchain, &device);

        let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(present_queue.family_index);
        let command_pool = unsafe { device.create_command_pool(&command_pool_create_info, None) }
            .expect("Failed to create renderer command pool");
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .command_buffer_count(1)
            .level(vk::CommandBufferLevel::PRIMARY)
            .build();
        let primary_command_buffer =
            unsafe { device.allocate_command_buffers(&command_buffer_allocate_info) }
                .expect("Failed to allocate primary command buffer")[0];

        let sync_objects = self.create_sync_objects(&device);

        let (descriptor_pool, descriptors) = self.create_descriptors(&device, &mut gpu_allocator);

        Renderer {
            clear_color: [0.0_f32, 0.0_f32, 0.0_f32, 1.0_f32],

            width: self.width,
            height: self.height,
            next_image_index: 0,

            debug_messenger,

            descriptors,
            descriptor_pool,
            sync_objects,
            primary_command_buffer,
            command_pool,
            swapchain_framebuffers,
            primary_render_pass,
            swapchain,
            present_queue,
            gpu_allocator: Some(gpu_allocator),
            device,
            physical_device,
            surface,
            instance,
            entry,
        }
    }
}

impl Renderer {
    pub fn begin_frame(&mut self) -> bool {
        unsafe {
            self.device
                .wait_for_fences(&[self.sync_objects.render_fence], true, u64::MAX)
        }
        .expect("Failed to wait for the render fence");
        unsafe { self.device.reset_fences(&[self.sync_objects.render_fence]) }
            .expect("Failed to reset the render fence");

        let next_image_index_maybe = unsafe {
            self.swapchain.loader.acquire_next_image(
                self.swapchain.handle,
                u64::MAX,
                self.sync_objects.present_semaphore,
                vk::Fence::null(),
            )
        };
        match next_image_index_maybe {
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                let fence_reset = vk::SubmitInfo::default();
                unsafe {
                    self.device.queue_submit(
                        self.present_queue.handle,
                        &[fence_reset],
                        self.sync_objects.render_fence,
                    )
                }
                .expect("Failed to reset fence");

                log::debug!(
                    "Returning early from begin_frame due to encountered VK_ERROR_OUT_OF_DATE_KHR"
                );

                false
            }
            Err(err) => panic!("Failed to acquire next swapchain image: {:?}", err),
            Ok((next_image_index, _)) => {
                self.next_image_index = next_image_index;
                let next_image_index: usize = next_image_index
                    .try_into()
                    .expect("Unsupported architecture");

                unsafe {
                    self.device.begin_command_buffer(
                        self.primary_command_buffer,
                        &vk::CommandBufferBeginInfo {
                            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                            ..Default::default()
                        },
                    )
                }
                .expect("Failed to start command buffer");

                let color_clear = vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: self.clear_color,
                    },
                };
                let depth_clear = vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0_f32,
                        stencil: 0,
                    },
                };
                let rp_begin_info = vk::RenderPassBeginInfo::builder()
                    .render_pass(self.primary_render_pass)
                    .framebuffer(self.swapchain_framebuffers[next_image_index])
                    .render_area(vk::Rect2D {
                        extent: vk::Extent2D {
                            width: self.width,
                            height: self.height,
                        },
                        ..Default::default()
                    })
                    .clear_values(&[color_clear, depth_clear])
                    .build();

                unsafe {
                    self.device.cmd_begin_render_pass(
                        self.primary_command_buffer,
                        &rp_begin_info,
                        vk::SubpassContents::INLINE,
                    )
                };

                true
            }
        }
    }

    pub fn end_frame(&self) {
        unsafe { self.device.cmd_end_render_pass(self.primary_command_buffer) };
        unsafe { self.device.end_command_buffer(self.primary_command_buffer) }
            .expect("Failed to record command buffer");

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&[self.sync_objects.present_semaphore])
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .command_buffers(&[self.primary_command_buffer])
            .signal_semaphores(&[self.sync_objects.render_semaphore])
            .build();
        unsafe {
            self.device.queue_submit(
                self.present_queue.handle,
                &[submit_info],
                self.sync_objects.render_fence,
            )
        }
        .expect("Failed to submit command buffer to present queue");

        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&[self.sync_objects.render_semaphore])
            .swapchains(&[self.swapchain.handle])
            .image_indices(&[self.next_image_index])
            .build();
        let result = unsafe {
            self.swapchain
                .loader
                .queue_present(self.present_queue.handle, &present_info)
        };

        match result {
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                log::debug!("Ignoring VK_ERROR_OUT_OF_DATE_KHR in end_frame")
            }
            Err(err) => panic!("Failed to present new image, {:?}", err),
            Ok(_) => (),
        };
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    fn recreate_swapchain(&mut self) {}
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device_wait_idle()
                .expect("Failed to wait for device");

            self.device
                .destroy_descriptor_set_layout(self.descriptors[1].layout, None);
            if let Some(gpu_allocator) = self.gpu_allocator.as_mut() {
                if let Some(time_buffer) = self.descriptors[0].buffer.take() {
                    time_buffer.destroy(&self.device, gpu_allocator);
                }
            }
            self.device
                .destroy_descriptor_set_layout(self.descriptors[0].layout, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);

            self.device
                .destroy_semaphore(self.sync_objects.render_semaphore, None);
            self.device
                .destroy_semaphore(self.sync_objects.present_semaphore, None);
            self.device
                .destroy_fence(self.sync_objects.render_fence, None);

            self.device.destroy_command_pool(self.command_pool, None);

            for framebuffer in &self.swapchain_framebuffers {
                self.device.destroy_framebuffer(*framebuffer, None);
            }

            self.device
                .destroy_render_pass(self.primary_render_pass, None);

            if let Some(gpu_allocator) = self.gpu_allocator.as_mut() {
                let swapchain_depth_image = std::mem::take(&mut self.swapchain.depth_image);
                swapchain_depth_image.destroy(&self.device, gpu_allocator);
            }

            for image_view in &self.swapchain.image_views {
                self.device.destroy_image_view(*image_view, None);
            }

            self.swapchain
                .loader
                .destroy_swapchain(self.swapchain.handle, None);

            if let Some(gpu_allocator) = self.gpu_allocator.take() {
                drop(gpu_allocator);
            }

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
