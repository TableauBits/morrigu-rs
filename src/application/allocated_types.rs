use ash::vk;
use gpu_allocator::vulkan::{Allocation, Allocator};

#[derive(Default)]
pub struct AllocatedImage {
    view: vk::ImageView,
    allocation: Allocation,
    handle: vk::Image,
}

impl AllocatedImage {
    pub fn destroy(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe { device.destroy_image_view(self.view, None) };
        allocator
            .free(self.allocation)
            .expect("Failed to free image memory!");
        unsafe { device.destroy_image(self.handle, None) };
    }
}

pub struct AllocatedImageBuilder<'a> {
    image_create_info_builder: vk::ImageCreateInfoBuilder<'a>,
    image_view_create_info_builder: vk::ImageViewCreateInfoBuilder<'a>,
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
        allocator: &mut gpu_allocator::vulkan::Allocator,
    ) -> AllocatedImage {
        let create_info = self.image_create_info_builder.build();
        let handle =
            unsafe { device.create_image(&create_info, None) }.expect("Failed to create image!");

        let memory_requirements = unsafe { device.get_image_memory_requirements(handle) };

        let allocation = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "Image allocation",
                requirements: memory_requirements,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
            })
            .expect("Failed to allocate image memory!");

        unsafe { device.bind_image_memory(handle, allocation.memory(), allocation.offset()) }
            .expect("Failed to bind image memory!");

        let view_create_info = self.image_view_create_info_builder.image(handle).build();

        let view = unsafe { device.create_image_view(&view_create_info, None) }
            .expect("Failed to create image view!");

        AllocatedImage {
            view,
            allocation,
            handle,
        }
    }
}

impl<'a> AllocatedImage {
    pub fn builder(extent: vk::Extent3D) -> AllocatedImageBuilder<'a> {
        AllocatedImageBuilder::new(extent)
    }
}
