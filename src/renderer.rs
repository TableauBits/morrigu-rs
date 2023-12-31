use crate::{
    allocated_types::{AllocatedBuffer, AllocatedBufferBuilder, AllocatedImage},
    math_types::Vec4,
    texture::Texture,
    utils::{CommandUploader, ImmediateCommandError, ThreadSafeRef},
};

use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{Surface, Swapchain},
    },
    vk::{self, PhysicalDeviceType},
    Entry, Instance,
};
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocationSizes,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::window::Window;

use std::{
    cmp::Ordering,
    ffi::{CStr, CString},
    mem,
    sync::MutexGuard,
};

#[cfg(debug_assertions)]
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> u32 {
    let callback_data_deref = *callback_data;
    let message_id_str = callback_data_deref.message_id_number.to_string();
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

pub struct QueueInfo {
    pub handle: vk::Queue,
    pub family_index: u32,
}

struct SurfaceInfo {
    handle: vk::SurfaceKHR,
    format: vk::SurfaceFormatKHR,
    loader: Surface,
}

struct SwapchainInfo {
    handle: vk::SwapchainKHR,
    #[allow(dead_code)] // Unused for now, but need to keep these alive
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    depth_image: AllocatedImage,
    preferred_present_mode: vk::PresentModeKHR,
    loader: Swapchain,
    extent: vk::Extent2D,
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

pub(crate) struct DescriptorInfo {
    pub(crate) handle: vk::DescriptorSet,
    pub(crate) layout: vk::DescriptorSetLayout,
    pub(crate) buffer: Option<AllocatedBuffer>,
}

pub struct Renderer {
    pub clear_color: [f32; 4],

    needs_resize: bool,
    window_width: u32,
    window_height: u32,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    next_image_index: u32,

    #[allow(dead_code)]
    debug_messenger: Option<DebugMessengerInfo>,

    pub(crate) default_texture_ref: ThreadSafeRef<Texture>,

    pub(crate) command_uploader: CommandUploader,

    pub(crate) descriptors: [DescriptorInfo; 2],
    descriptor_pool: vk::DescriptorPool,
    sync_objects: SyncObjects,
    pub(crate) primary_command_buffer: vk::CommandBuffer,
    command_pool: vk::CommandPool,
    swapchain_framebuffers: Vec<vk::Framebuffer>,
    pub(crate) primary_render_pass: vk::RenderPass,
    swapchain: SwapchainInfo,
    pub graphics_queue: QueueInfo,
    pub allocator: Option<ThreadSafeRef<Allocator>>,
    pub device: ash::Device,
    pub device_properties: vk::PhysicalDeviceProperties,
    physical_device: vk::PhysicalDevice,
    surface: SurfaceInfo,
    instance: ash::Instance,
    #[allow(dead_code)]
    // This field is never read, but we need to keep it alive longer than the instance
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

    rt_requested: bool,
}

#[allow(clippy::too_many_arguments)]
fn create_swapchain(
    mut width: u32,
    mut height: u32,
    preferred_present_mode: vk::PresentModeKHR,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    surface: &SurfaceInfo,
    allocator: &mut Allocator,
) -> SwapchainInfo {
    let capabilities = unsafe {
        surface.loader.get_physical_device_surface_capabilities(physical_device, surface.handle)
    }
    .expect("Failed to query surface capabilities");
    let mut requested_image_count = capabilities.min_image_count + 1;
    if capabilities.max_image_count > 0 && requested_image_count > capabilities.max_image_count {
        requested_image_count = capabilities.max_image_count;
    }

    let surface_extent = match capabilities.current_extent.width {
        std::u32::MAX => vk::Extent2D { width, height },
        _ => {
            width = capabilities.current_extent.width;
            height = capabilities.current_extent.height;

            capabilities.current_extent
        }
    };

    let present_modes = unsafe {
        surface.loader.get_physical_device_surface_present_modes(physical_device, surface.handle)
    }
    .expect("Failed to query surface present modes");
    let present_mode = present_modes
        .iter()
        .cloned()
        .find(|&mode| mode == preferred_present_mode)
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
        .pre_transform(capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .image_array_layers(1);

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
            .image(image);
        unsafe { device.create_image_view(&create_view_info, None) }
            .expect("Failed to ceate swapchain image views")
    };

    let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain) }
        .expect("Failed to get swapchain images");
    let swapchain_image_views = swapchain_images.iter().map(image_view_creator).collect();

    let depth_extent = vk::Extent3D {
        width,
        height,
        depth: 1,
    };

    let depth_image_create_info_builder = vk::ImageCreateInfo::builder()
        .extent(depth_extent)
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::D32_SFLOAT)
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let depth_image_handle = unsafe { device.create_image(&depth_image_create_info_builder, None) }
        .expect("Failed to create image");

    let memory_requirements = unsafe { device.get_image_memory_requirements(depth_image_handle) };
    let depth_allocation = allocator
        .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
            name: "Depth image allocation",
            requirements: memory_requirements,
            location: gpu_allocator::MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: gpu_allocator::vulkan::AllocationScheme::DedicatedImage(
                depth_image_handle,
            ),
        })
        .expect("Failed to allocate depth image");
    unsafe {
        device.bind_image_memory(
            depth_image_handle,
            depth_allocation.memory(),
            depth_allocation.offset(),
        )
    }
    .expect("Failed to bind depth image memory");

    let depth_image_view_create_info_builder = vk::ImageViewCreateInfo::builder()
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::D32_SFLOAT)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::DEPTH,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .image(depth_image_handle);
    let depth_image_view =
        unsafe { device.create_image_view(&depth_image_view_create_info_builder, None) }
            .expect("Failed to create depth image view");

    SwapchainInfo {
        handle: swapchain,
        images: swapchain_images,
        image_views: swapchain_image_views,
        depth_image: AllocatedImage {
            handle: depth_image_handle,
            view: depth_image_view,
            allocation: Some(depth_allocation),
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            format: depth_image_create_info_builder.format,
            extent: depth_extent,
            layer_count: 1,
        },
        preferred_present_mode,
        loader: swapchain_loader,
        extent: surface_extent,
    }
}

fn create_framebuffers(
    width: u32,
    height: u32,
    render_pass: vk::RenderPass,
    swapchain: &SwapchainInfo,
    device: &ash::Device,
) -> Vec<vk::Framebuffer> {
    let mut framebuffer_create_info = vk::FramebufferCreateInfo::builder()
        .render_pass(render_pass)
        .width(width)
        .height(height)
        .layers(1);
    framebuffer_create_info.attachment_count = 2;

    let mut framebuffers = vec![];
    for swapchain_image_view in swapchain.image_views.clone() {
        let attachments = [swapchain_image_view, swapchain.depth_image.view];
        framebuffer_create_info.p_attachments = attachments.as_ptr();
        framebuffers.push(
            unsafe { device.create_framebuffer(&framebuffer_create_info, None) }
                .expect("Failed to create framebuffer"),
        );
    }

    framebuffers
}

impl<'a> RendererBuilder<'a> {
    fn create_instance(&self, entry: &ash::Entry) -> Instance {
        let engine_name = CString::new("Morrigu").unwrap();
        let app_info = vk::ApplicationInfo::builder()
            .application_name(self.application_name.as_c_str())
            .application_version(self.application_version)
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::make_api_version(0, 1, 2, 0));

        #[allow(unused_mut)]
        let mut required_extensions =
            ash_window::enumerate_required_extensions(self.window_handle.raw_display_handle())
                .expect("Failed to query extensions")
                .to_vec();

        #[allow(unused_assignments)]
        #[allow(unused_mut)]
        let mut raw_layer_names = vec![];
        #[cfg(debug_assertions)]
        {
            let layer_names =
                [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];
            raw_layer_names = layer_names.iter().map(|layer| layer.as_ptr()).collect();

            required_extensions.push(ash::extensions::ext::DebugUtils::name().as_ptr());
        }

        let instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_layer_names(&raw_layer_names)
            .enabled_extension_names(&required_extensions);
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
                .pfn_user_callback(Some(vulkan_debug_callback));

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
        let mut physical_devices = unsafe { instance.enumerate_physical_devices() }
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
                    let supports_compute = device_queue_info
                        .queue_flags
                        .contains(vk::QueueFlags::COMPUTE);
                    let is_compatible_with_surface = unsafe {
                        surface_loader.get_physical_device_surface_support(
                            raw_physical_device,
                            queue_index as u32,
                            surface,
                        )
                    }
                    .expect("Failed to query surface compatibility");

                    let meets_rt_requirements = true;
                    // if self.rt_requested {

                    // }

                    if supports_required_version
                        && supports_graphics
                        && supports_compute
                        && is_compatible_with_surface
                        && meets_rt_requirements
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

        physical_devices.sort_unstable_by(|a, b| {
            let device_a_info = unsafe { instance.get_physical_device_properties(*a) };
            let device_b_info = unsafe { instance.get_physical_device_properties(*b) };

            let mut ordering = Ordering::Equal;
            if device_a_info.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
                && device_b_info.device_type != vk::PhysicalDeviceType::DISCRETE_GPU
            {
                ordering = Ordering::Less;
            }
            if device_b_info.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
                && device_a_info.device_type != vk::PhysicalDeviceType::DISCRETE_GPU
            {
                ordering = Ordering::Greater;
            }

            ordering
        });
        log::debug!("Physical device list (sorted):");
        for device in &physical_devices {
            let device_info = unsafe { instance.get_physical_device_properties(*device) };

            log::debug!(
                "\t{}: {}",
                unsafe {
                    CStr::from_ptr(device_info.device_name.as_ptr())
                        .to_str()
                        .unwrap_or("Invalid name")
                },
                device_type_to_str(device_info.device_type)
            );
        }
        physical_devices
            .iter()
            .find_map(device_selector)
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
        let mut raw_extensions_names = vec![Swapchain::name().as_ptr()];
        let features = vk::PhysicalDeviceFeatures::default();
        let priorities = [1.0];

        if self.rt_requested {
            // For rt acceleration structures
            raw_extensions_names.push(ash::extensions::khr::AccelerationStructure::name().as_ptr());
            // For vkCmdTraceRaysKHR
            raw_extensions_names.push(ash::extensions::khr::RayTracingPipeline::name().as_ptr());
            // Required by RayTracingPipeline
            raw_extensions_names
                .push(ash::extensions::khr::DeferredHostOperations::name().as_ptr());
        }

        let queue_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priorities);

        let mut device_create_info = vk::DeviceCreateInfo::builder()
            .enabled_features(&features)
            .enabled_extension_names(&raw_extensions_names)
            .queue_create_infos(std::slice::from_ref(&queue_info));

        let mut as_features = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
        let mut rtp_features = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
        if self.rt_requested {
            device_create_info = device_create_info.push_next(&mut as_features);
            device_create_info = device_create_info.push_next(&mut rtp_features);
        }

        let device = unsafe { instance.create_device(physical_device, &device_create_info, None) }
            .expect("Failed to create logial device");

        if self.rt_requested {
            log::info!("Ray tracing extensions features:");
            log::info!("\t acceleration structure: {:?}", as_features);
            log::info!("\t ray tracing pipeline: {:?}", rtp_features);
        }

        device
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
            allocation_sizes: AllocationSizes::default(),
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

        let color_attachment_refs = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
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
            .color_attachments(&color_attachment_refs)
            .depth_stencil_attachment(&depth_attachment_ref);

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
            .subpasses(std::slice::from_ref(&subpass_description));

        unsafe { device.create_render_pass(&renderpass_info, None) }
            .expect("Failed to create render pass")
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
            }]);
        let descriptor_pool = unsafe { device.create_descriptor_pool(&descriptor_pool_info, None) }
            .expect("Failed to create descriptor pool");

        let level_0_bindings = [vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            ..Default::default()
        }];
        let level_0_layout_info =
            vk::DescriptorSetLayoutCreateInfo::builder().bindings(&level_0_bindings);
        let level_0_layout =
            unsafe { device.create_descriptor_set_layout(&level_0_layout_info, None) }
                .expect("Failed to create descriptor set 0 layout");
        let level_0_allocation_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&level_0_layout));
        let level_0_handle = unsafe { device.allocate_descriptor_sets(&level_0_allocation_info) }
            .expect("Failed to allocate level 0 descriptor")[0];
        let time_buffer_size: u64 = mem::size_of::<Vec4>().try_into().unwrap();
        let time_buffer = AllocatedBufferBuilder::uniform_buffer_default(time_buffer_size)
            .build_internal(device, allocator)
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

        let level_1_layout_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&[]);
        let level_1_layout =
            unsafe { device.create_descriptor_set_layout(&level_1_layout_info, None) }
                .expect("Failed to create descriptor set 0 layout");
        let level_1_allocation_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&level_1_layout));
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

            rt_requested: false,
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

    pub fn request_ray_tracing(mut self, request_ray_tracing: bool) -> Self {
        self.rt_requested = request_ray_tracing;
        self
    }

    pub fn build(mut self) -> ThreadSafeRef<Renderer> {
        let entry = Entry::linked();
        let instance = self.create_instance(&entry);
        let debug_messenger = self.create_debug_messenger(&entry, &instance);

        let surface_handle = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                self.window_handle.raw_display_handle(),
                self.window_handle.raw_window_handle(),
                None,
            )
            .expect("Failed to create rendering surface")
        };
        let surface_loader = Surface::new(&entry, &instance);

        let required_api_version = (1, 1, 0);
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
        let surface = SurfaceInfo {
            handle: surface_handle,
            format: surface_format,
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
        let graphics_queue = QueueInfo {
            handle: unsafe { device.get_device_queue(queue_family_index, 0) },
            family_index: queue_family_index,
        };

        let mut command_uploader = CommandUploader::new(&device, queue_family_index)
            .expect("Failed to create a command uploader");

        let mut gpu_allocator =
            self.create_allocator(instance.clone(), physical_device, device.clone());

        let swapchain = create_swapchain(
            self.width,
            self.height,
            self.preferred_present_mode,
            &instance,
            physical_device,
            &device,
            &surface,
            &mut gpu_allocator,
        );
        self.width = swapchain.extent.width;
        self.height = swapchain.extent.height;

        let primary_render_pass =
            self.create_render_passes(&surface, &swapchain.depth_image, &device);

        let swapchain_framebuffers = create_framebuffers(
            self.width,
            self.height,
            primary_render_pass,
            &swapchain,
            &device,
        );

        let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(graphics_queue.family_index);
        let command_pool = unsafe { device.create_command_pool(&command_pool_create_info, None) }
            .expect("Failed to create renderer command pool");
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .command_buffer_count(1)
            .level(vk::CommandBufferLevel::PRIMARY);
        let primary_command_buffer =
            unsafe { device.allocate_command_buffers(&command_buffer_allocate_info) }
                .expect("Failed to allocate primary command buffer")[0];

        let sync_objects = self.create_sync_objects(&device);

        let (descriptor_pool, descriptors) = self.create_descriptors(&device, &mut gpu_allocator);

        let default_texture_ref = Texture::builder()
            .build_default_internal(
                &device,
                graphics_queue.handle,
                &mut gpu_allocator,
                &mut command_uploader,
            )
            .expect("Default texture creation failed");

        ThreadSafeRef::new(Renderer {
            clear_color: [0.0_f32, 0.0_f32, 0.0_f32, 1.0_f32],

            needs_resize: false,
            window_width: self.width,
            window_height: self.height,
            framebuffer_width: self.width,
            framebuffer_height: self.height,
            next_image_index: 0,

            debug_messenger,

            default_texture_ref,

            command_uploader,
            descriptors,
            descriptor_pool,
            sync_objects,
            primary_command_buffer,
            command_pool,
            swapchain_framebuffers,
            primary_render_pass,
            swapchain,
            graphics_queue,
            allocator: Some(ThreadSafeRef::new(gpu_allocator)),
            device,
            device_properties,
            physical_device,
            surface,
            instance,
            entry,
        })
    }
}

impl Renderer {
    pub fn allocator(&self) -> MutexGuard<Allocator> {
        self.allocator
            .as_ref()
            .expect("Allocator was not initialized")
            .lock()
    }

    pub fn default_texture(&self) -> ThreadSafeRef<Texture> {
        self.default_texture_ref.clone()
    }

    pub(crate) fn begin_frame(&mut self) -> bool {
        if self.window_width == 0 || self.window_height == 0 {
            return false;
        }

        unsafe {
            self.device
                .wait_for_fences(&[self.sync_objects.render_fence], true, u64::MAX)
        }
        .expect("Failed to wait for the render fence");

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
                self.recreate_swapchain();
                false
            }
            Err(err) => panic!("Failed to acquire next swapchain image: {:?}", err),
            Ok((next_image_index, is_suboptimal)) => {
                if is_suboptimal {
                    log::debug!("Suboptimal frame image acquired (probably due to resize)");
                }

                unsafe { self.device.reset_fences(&[self.sync_objects.render_fence]) }
                    .expect("Failed to reset the render fence");

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

                let clear_values = [
                    vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: self.clear_color,
                        },
                    },
                    vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: 1.0_f32,
                            stencil: 0,
                        },
                    },
                ];
                let rp_begin_info = vk::RenderPassBeginInfo::builder()
                    .render_pass(self.primary_render_pass)
                    .framebuffer(self.swapchain_framebuffers[next_image_index])
                    .render_area(vk::Rect2D {
                        extent: vk::Extent2D {
                            width: self.framebuffer_width,
                            height: self.framebuffer_height,
                        },
                        ..Default::default()
                    })
                    .clear_values(&clear_values);

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

    pub(crate) fn end_frame(&mut self) {
        unsafe { self.device.cmd_end_render_pass(self.primary_command_buffer) };
        unsafe { self.device.end_command_buffer(self.primary_command_buffer) }
            .expect("Failed to record command buffer");

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(std::slice::from_ref(&self.sync_objects.present_semaphore))
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .command_buffers(std::slice::from_ref(&self.primary_command_buffer))
            .signal_semaphores(std::slice::from_ref(&self.sync_objects.render_semaphore));
        unsafe {
            self.device.queue_submit(
                self.graphics_queue.handle,
                &[submit_info.build()],
                self.sync_objects.render_fence,
            )
        }
        .expect("Failed to submit command buffer to present queue");

        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(std::slice::from_ref(&self.sync_objects.render_semaphore))
            .swapchains(std::slice::from_ref(&self.swapchain.handle))
            .image_indices(std::slice::from_ref(&self.next_image_index));
        let result = unsafe {
            self.swapchain
                .loader
                .queue_present(self.graphics_queue.handle, &present_info)
        };

        match result {
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Ok(true) => {
                self.recreate_swapchain();
            }
            Ok(false) => {
                if self.needs_resize {
                    self.needs_resize = false;
                    self.recreate_swapchain();
                }
            }
            Err(err) => panic!("Failed to present new image, {:?}", err),
        };
    }

    pub(crate) fn on_resize(&mut self, width: u32, height: u32) {
        self.needs_resize = true;
        self.window_width = width;
        self.window_height = height;
    }

    fn recreate_swapchain(&mut self) {
        unsafe { self.device.device_wait_idle() }.expect("Failed to wait for device");

        // 1. Destroy all VK objects that will need to be recreated with the new swapchain.
        //    - all framebuffers
        for framebuffer in &self.swapchain_framebuffers {
            unsafe { self.device.destroy_framebuffer(*framebuffer, None) };
        }

        //    - the depth image
        let mut swapchain_depth_image = std::mem::take(&mut self.swapchain.depth_image);
        swapchain_depth_image.destroy(self);

        //    - the swapchain image views
        for image_view in &self.swapchain.image_views {
            unsafe { self.device.destroy_image_view(*image_view, None) };
        }

        //    - and finally the swapchain itself
        unsafe {
            self.swapchain
                .loader
                .destroy_swapchain(self.swapchain.handle, None)
        };

        // 2. Recreate all necessary VK objects
        //    - the swapchain itself
        //    - the swapchain image views
        //    - the depth image
        self.swapchain = create_swapchain(
            self.window_width,
            self.window_height,
            self.swapchain.preferred_present_mode,
            &self.instance,
            self.physical_device,
            &self.device,
            &self.surface,
            &mut self.allocator.as_ref().unwrap().lock(),
        );

        //    - and finally the framebuffers
        self.framebuffer_width = std::cmp::min(self.window_width, self.swapchain.extent.width);
        self.framebuffer_height = std::cmp::min(self.window_height, self.swapchain.extent.height);
        self.swapchain_framebuffers = create_framebuffers(
            self.framebuffer_width,
            self.framebuffer_height,
            self.primary_render_pass,
            &self.swapchain,
            &self.device,
        );
    }

    pub fn immediate_command<F>(&self, function: F) -> Result<(), ImmediateCommandError>
    where
        F: FnOnce(&vk::CommandBuffer),
    {
        self.command_uploader
            .immediate_command(&self.device, self.graphics_queue.handle, function)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device_wait_idle()
                .expect("Failed to wait for device");

            self.default_texture_ref
                .lock()
                .destroy_internal(&self.device, &mut self.allocator());

            self.device
                .destroy_descriptor_set_layout(self.descriptors[1].layout, None);
            if let Some(mut time_buffer) = self.descriptors[0].buffer.take() {
                time_buffer.destroy(&self.device, &mut self.allocator());
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

            let mut swapchain_depth_image = std::mem::take(&mut self.swapchain.depth_image);
            swapchain_depth_image.destroy(self);

            for image_view in &self.swapchain.image_views {
                self.device.destroy_image_view(*image_view, None);
            }

            self.swapchain
                .loader
                .destroy_swapchain(self.swapchain.handle, None);

            if let Some(allocator) = self.allocator.take() {
                drop(allocator);
            }

            let command_uploader = std::mem::take(&mut self.command_uploader);
            command_uploader.destroy(&self.device);

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
