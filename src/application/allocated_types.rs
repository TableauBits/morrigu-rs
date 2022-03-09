use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, Allocator};

#[derive(Default)]
pub struct AllocatedBuffer {
    pub handle: vk::Buffer,
    allocation: Allocation,
}

impl AllocatedBuffer {
    pub fn destroy(self, device: &ash::Device, allocator: &mut Allocator) {
        allocator
            .free(self.allocation)
            .expect("Failed to free buffer memory");
        unsafe { device.destroy_buffer(self.handle, None) };
    }
}

pub struct AllocatedBufferBuilder {
    pub size: u64,
    pub usage: vk::BufferUsageFlags,
    pub memory_location: gpu_allocator::MemoryLocation,
}

impl AllocatedBufferBuilder {
    pub fn uniform_buffer_default(size: u64) -> AllocatedBufferBuilder {
        AllocatedBufferBuilder {
            size,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            memory_location: gpu_allocator::MemoryLocation::CpuToGpu,
        }
    }

    pub fn with_usage(mut self, usage: vk::BufferUsageFlags) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_memory_location(mut self, memory_location: gpu_allocator::MemoryLocation) -> Self {
        self.memory_location = memory_location;
        self
    }

    pub fn build(
        self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<AllocatedBuffer, Box<dyn std::error::Error>> {
        let buffer_info = vk::BufferCreateInfo {
            size: self.size,
            usage: self.usage,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let handle = unsafe { device.create_buffer(&buffer_info, None) }?;

        let memory_req = unsafe { device.get_buffer_memory_requirements(handle) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "buffer",
            requirements: memory_req,
            location: self.memory_location,
            linear: true,
        })?;

        unsafe { device.bind_buffer_memory(handle, allocation.memory(), allocation.offset()) }?;

        Ok(AllocatedBuffer { handle, allocation })
    }
}

#[derive(Default)]
pub struct AllocatedImage {
    pub view: vk::ImageView,
    allocation: Allocation,
    pub handle: vk::Image,
    pub format: vk::Format,
}

impl AllocatedImage {
    pub fn destroy(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe { device.destroy_image_view(self.view, None) };
        allocator
            .free(self.allocation)
            .expect("Failed to free image memory");
        unsafe { device.destroy_image(self.handle, None) };
    }
}

pub struct AllocatedImageBuilder<'a> {
    pub image_create_info_builder: vk::ImageCreateInfoBuilder<'a>,
    pub image_view_create_info_builder: vk::ImageViewCreateInfoBuilder<'a>,
}

impl<'a> AllocatedImageBuilder<'a> {
    pub fn new(extent: vk::Extent3D) -> Self {
        let image_create_info_builder = vk::ImageCreateInfo::builder().extent(extent);
        let image_view_create_info_builder = vk::ImageViewCreateInfo::builder();

        AllocatedImageBuilder {
            image_create_info_builder,
            image_view_create_info_builder,
        }
    }

    pub fn depth_image_default(mut self) -> Self {
        self.image_create_info_builder = self
            .image_create_info_builder
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::D16_UNORM)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        self.image_view_create_info_builder = self
            .image_view_create_info_builder
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::D16_UNORM)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        self
    }

    pub fn build(
        self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<AllocatedImage, Box<dyn std::error::Error>> {
        let create_info = self.image_create_info_builder.build();
        let handle =
            unsafe { device.create_image(&create_info, None) }.expect("Failed to create image");

        let memory_requirements = unsafe { device.get_image_memory_requirements(handle) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "Image allocation",
            requirements: memory_requirements,
            location: gpu_allocator::MemoryLocation::GpuOnly,
            linear: false,
        })?;

        unsafe { device.bind_image_memory(handle, allocation.memory(), allocation.offset()) }?;

        let view_create_info = self.image_view_create_info_builder.image(handle).build();
        let view = unsafe { device.create_image_view(&view_create_info, None) }?;

        Ok(AllocatedImage {
            view,
            allocation,
            handle,
            format: create_info.format,
        })
    }
}

impl<'a> AllocatedImage {
    pub fn builder(extent: vk::Extent3D) -> AllocatedImageBuilder<'a> {
        AllocatedImageBuilder::new(extent)
    }
}
