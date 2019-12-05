//! World rendering

use super::buffers::MultiBuffer;
use voxel_rs_common::world::chunk::ChunkPos;
use image::{ImageBuffer, Rgba};
use voxel_rs_common::block::BlockMesh;
use super::init::{load_glsl_shader, create_default_pipeline};
use crate::window::WindowBuffers;
use super::world::meshing_worker::MeshingWorker;
use crate::texture::load_image;
use super::frustum::Frustum;
use voxel_rs_common::debug::send_debug_info;
use voxel_rs_common::world::{World, BlockPos};

mod meshing;
mod meshing_worker;
mod skybox;

/// All the state necessary to render the world.
pub struct WorldRenderer {
    // Chunk meshing
    meshing_worker: MeshingWorker,
    // View-projection matrix
    uniform_view_proj: wgpu::Buffer,
    // Model matrix
    uniform_model: wgpu::Buffer,
    // Chunk rendering
    chunk_index_buffers: MultiBuffer<ChunkPos, u32>,
    chunk_vertex_buffers: MultiBuffer<ChunkPos, ChunkVertex>,
    chunk_pipeline: wgpu::RenderPipeline,
    chunk_bind_group: wgpu::BindGroup,
    // Skybox rendering
    skybox_index_buffer: wgpu::Buffer,
    skybox_vertex_buffer: wgpu::Buffer,
    skybox_pipeline: wgpu::RenderPipeline,
    // View-proj and model bind group
    vpm_bind_group: wgpu::BindGroup,
    // Targeted block rendering
    target_vertex_buffer: wgpu::Buffer,
    target_pipeline: wgpu::RenderPipeline,
}

impl WorldRenderer {
    pub fn new(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture_atlas: ImageBuffer<Rgba<u8>, Vec<u8>>,
        block_meshes: Vec<BlockMesh>,
    ) -> Self {
        let mut compiler = shaderc::Compiler::new().expect("Failed to create shader compiler");

        // Load texture atlas
        let texture_atlas = load_image(device, encoder, texture_atlas);
        let texture_atlas_view = texture_atlas.create_default_view();

        // Create uniform buffers
        let uniform_view_proj = device.create_buffer(&wgpu::BufferDescriptor {
            size: 64,
            usage: (wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST),
        });
        let uniform_model = device.create_buffer(&wgpu::BufferDescriptor {
            size: 64,
            usage: (wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST),
        });

        // Create uniform bind group
        let chunk_bind_group_layout = device.create_bind_group_layout(&CHUNK_BIND_GROUP_LAYOUT);
        let chunk_bind_group = create_chunk_bind_group(
            device,
            &chunk_bind_group_layout,
            &texture_atlas_view,
            &uniform_view_proj
        );

        // Create chunk pipeline
        let chunk_pipeline = {
            let vertex_shader =
                load_glsl_shader(&mut compiler, shaderc::ShaderKind::Vertex, "assets/shaders/world.vert");
            let fragment_shader =
                load_glsl_shader(&mut compiler, shaderc::ShaderKind::Fragment, "assets/shaders/world.frag");

            create_default_pipeline(
                device,
                &chunk_bind_group_layout,
                vertex_shader.as_binary(),
                fragment_shader.as_binary(),
                wgpu::PrimitiveTopology::TriangleList,
                wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<ChunkVertex>() as u64,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &CHUNK_VERTEX_ATTRIBUTES,
                },
                false,
            )
        };

        // Create skybox vertex and index buffers
        let (skybox_vertex_buffer, skybox_index_buffer) = self::skybox::create_skybox(device);

        // Create skybox bind group
        let vpm_bind_group_layout = device.create_bind_group_layout(&SKYBOX_BIND_GROUP_LAYOUT);
        let vpm_bind_group = create_vpm_bind_group(device, &vpm_bind_group_layout, &uniform_view_proj, &uniform_model);

        // Create skybox pipeline
        let skybox_pipeline = {
            let vertex_shader =
                load_glsl_shader(&mut compiler, shaderc::ShaderKind::Vertex, "assets/shaders/skybox.vert");
            let fragment_shader =
                load_glsl_shader(&mut compiler, shaderc::ShaderKind::Fragment, "assets/shaders/skybox.frag");

            create_default_pipeline(
                device,
                &vpm_bind_group_layout,
                vertex_shader.as_binary(),
                fragment_shader.as_binary(),
                wgpu::PrimitiveTopology::TriangleList,
                wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<SkyboxVertex>() as u64,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &SKYBOX_VERTEX_ATTRIBUTES,
                },
                false,
            )
        };

        // Create target buffer and pipeline
        let target_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            size: 8 * std::mem::size_of::<SkyboxVertex>() as u64,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });
        let target_pipeline = {
            let vertex_shader =
                load_glsl_shader(&mut compiler, shaderc::ShaderKind::Vertex, "assets/shaders/target.vert");
            let fragment_shader =
                load_glsl_shader(&mut compiler, shaderc::ShaderKind::Fragment, "assets/shaders/target.frag");

            create_default_pipeline(
                device,
                &vpm_bind_group_layout,
                vertex_shader.as_binary(),
                fragment_shader.as_binary(),
                wgpu::PrimitiveTopology::LineList,
                wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<SkyboxVertex>() as u64,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &SKYBOX_VERTEX_ATTRIBUTES,
                },
                false,
            )
        };

        Self {
            meshing_worker: MeshingWorker::new(block_meshes),
            uniform_view_proj,
            uniform_model,
            chunk_index_buffers: MultiBuffer::with_capacity(device, 1000, wgpu::BufferUsage::INDEX),
            chunk_vertex_buffers: MultiBuffer::with_capacity(device, 1000, wgpu::BufferUsage::VERTEX),
            chunk_pipeline,
            chunk_bind_group,
            skybox_vertex_buffer,
            skybox_index_buffer,
            skybox_pipeline,
            vpm_bind_group,
            target_vertex_buffer,
            target_pipeline,
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        buffers: WindowBuffers,
        data: &crate::window::WindowData,
        frustum: &Frustum,
        enable_culling: bool,
        pointed_block: Option<(BlockPos, usize)>,
    ) {
        //============= RECEIVE CHUNK MESHES =============//
        for (pos, vertices, indices) in self.meshing_worker.get_processed_chunks() {
            if vertices.len() > 0 && indices.len() > 0 {
                self.chunk_vertex_buffers.update(
                    device,
                    encoder,
                    pos,
                    &vertices[..],
                );
                self.chunk_index_buffers.update(
                    device,
                    encoder,
                    pos,
                    &indices[..],
                );
            }
        }

        //============= RENDER =============//
        // TODO: what if win_h is 0 ?
        let aspect_ratio = {
            let winit::dpi::PhysicalSize {
                width: win_w,
                height: win_h,
            } = data.physical_window_size;
            win_w / win_h
        };

        let view_mat = frustum.get_view_matrix();
        let planes = frustum.get_planes(aspect_ratio);
        let view_proj_mat = frustum.get_view_projection(aspect_ratio);
        let opengl_to_wgpu = nalgebra::Matrix4::from([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, 0.5, 0.0],
            [0.0, 0.0, 0.5, 1.0],
        ]);
        let view_proj: [[f32; 4]; 4] = nalgebra::convert::<nalgebra::Matrix4<f64>, nalgebra::Matrix4<f32>>(opengl_to_wgpu * view_proj_mat).into();

        // Update view_proj matrix
        let src_buffer = device
            .create_buffer_mapped(4, wgpu::BufferUsage::COPY_SRC)
            .fill_from_slice(&view_proj);
        encoder.copy_buffer_to_buffer(&src_buffer, 0, &self.uniform_view_proj, 0, 64);

        // Draw all the chunks
        {
            let mut rpass = super::render::create_default_render_pass(encoder, buffers);
            rpass.set_pipeline(&self.chunk_pipeline);
            rpass.set_bind_group(0, &self.chunk_bind_group, &[]);
            rpass.set_vertex_buffers(0, &[(&self.chunk_vertex_buffers.get_buffer(), 0)]);
            rpass.set_index_buffer(&self.chunk_index_buffers.get_buffer(), 0);
            let mut count = 0;
            for chunk_pos in self.chunk_index_buffers.keys() {
                if !enable_culling || Frustum::contains_chunk(&planes, &view_mat, chunk_pos) {
                    count += 1;
                    let (index_pos, index_len) = self.chunk_index_buffers.get_pos_len(&chunk_pos).unwrap();
                    let (vertex_pos, _) = self.chunk_vertex_buffers.get_pos_len(&chunk_pos).unwrap();
                    rpass.draw_indexed(
                        (index_pos as u32)..((index_pos + index_len) as u32),
                        vertex_pos as i32,
                        0..1,
                    );
                }
            }
            send_debug_info(
                "Render",
                "renderedchunks",
                format!("{} chunks were rendered", count),
            );
        }

        // Draw the skybox
        {
            // Update model buffer
            let src_buffer = device
                .create_buffer_mapped(16, wgpu::BufferUsage::COPY_SRC)
                .fill_from_slice(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, frustum.position.x as f32, frustum.position.y as f32, frustum.position.z as f32, 1.0]);
            encoder.copy_buffer_to_buffer(&src_buffer, 0, &self.uniform_model, 0, 64);
            let mut rpass = super::render::create_default_render_pass(encoder, buffers);
            rpass.set_pipeline(&self.skybox_pipeline);
            rpass.set_bind_group(0, &self.vpm_bind_group, &[]);
            rpass.set_vertex_buffers(0, &[(&self.skybox_vertex_buffer, 0)]);
            rpass.set_index_buffer(&self.skybox_index_buffer, 0);
            rpass.draw_indexed(0..36, 0, 0..1);
        }

        // Draw the target if necessary
        if let Some((target_pos, target_face)) = pointed_block {
            // Generate the vertices
            // TODO: maybe check if they changed since last frame
            let src_buffer = device
                .create_buffer_mapped(8, wgpu::BufferUsage::COPY_SRC)
                .fill_from_slice(&create_target_vertices(target_face));
            encoder.copy_buffer_to_buffer(&src_buffer, 0, &self.target_vertex_buffer, 0, 8 * std::mem::size_of::<SkyboxVertex>() as u64);
            // Update model buffer
            let src_buffer = device
                .create_buffer_mapped(16, wgpu::BufferUsage::COPY_SRC)
                .fill_from_slice(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, target_pos.px as f32, target_pos.py as f32, target_pos.pz as f32, 1.0]);
            encoder.copy_buffer_to_buffer(&src_buffer, 0, &self.uniform_model, 0, 64);
            let mut rpass = super::render::create_default_render_pass(encoder, buffers);
            rpass.set_pipeline(&self.target_pipeline);
            rpass.set_bind_group(0, &self.vpm_bind_group, &[]);
            rpass.set_vertex_buffers(0, &[(&self.target_vertex_buffer, 0)]);
            rpass.draw(0..8, 0..1);
        }
    }

    pub fn update_chunk(
        &mut self,
        world: &World,
        pos: ChunkPos,
    ) {
        self.meshing_worker.enqueue_chunk(self::meshing::ChunkMeshData::create_from_world(world, pos));
    }

    pub fn remove_chunk(&mut self, pos: ChunkPos) {
        self.meshing_worker.dequeue_chunk(pos);
        self.chunk_vertex_buffers.remove(&pos);
        self.chunk_index_buffers.remove(&pos);
    }
}

/*========== CHUNK RENDERING ==========*/
/// Chunk vertex
#[derive(Debug, Clone, Copy)]
pub struct ChunkVertex {
    pub pos: [f32; 3],
    pub texture_top_left: [f32; 2],
    pub texture_size: [f32; 2],
    pub texture_max_uv: [f32; 2],
    pub texture_uv: [f32; 2],
    pub occl_and_face: u32,
}

/// Chunk vertex attributes
const CHUNK_VERTEX_ATTRIBUTES: [wgpu::VertexAttributeDescriptor; 6] = [
    wgpu::VertexAttributeDescriptor {
        shader_location: 0,
        format: wgpu::VertexFormat::Float3,
        offset: 0,
    },
    wgpu::VertexAttributeDescriptor {
        shader_location: 1,
        format: wgpu::VertexFormat::Float2,
        offset: 4 * 3,
    },
    wgpu::VertexAttributeDescriptor {
        shader_location: 2,
        format: wgpu::VertexFormat::Float2,
        offset: 4 * (3 + 2),
    },
    wgpu::VertexAttributeDescriptor {
        shader_location: 3,
        format: wgpu::VertexFormat::Float2,
        offset: 4 * (3 + 2 + 2),
    },
    wgpu::VertexAttributeDescriptor {
        shader_location: 4,
        format: wgpu::VertexFormat::Float2,
        offset: 4 * (3 + 2 + 2 + 2),
    },
    wgpu::VertexAttributeDescriptor {
        shader_location: 5,
        format: wgpu::VertexFormat::Uint,
        offset: 4 * (3 + 2 + 2 + 2 + 2),
    },
];

const CHUNK_BIND_GROUP_LAYOUT: wgpu::BindGroupLayoutDescriptor<'static> = wgpu::BindGroupLayoutDescriptor {
    bindings: &[
        wgpu::BindGroupLayoutBinding {
            binding: 0,
            visibility: wgpu::ShaderStage::VERTEX,
            ty: wgpu::BindingType::UniformBuffer { dynamic: false },
        },
        wgpu::BindGroupLayoutBinding {
            binding: 1,
            visibility: wgpu::ShaderStage::FRAGMENT,
            ty: wgpu::BindingType::Sampler,
        },
        wgpu::BindGroupLayoutBinding {
            binding: 2,
            visibility: wgpu::ShaderStage::FRAGMENT,
            ty: wgpu::BindingType::SampledTexture {
                multisampled: false,
                dimension: wgpu::TextureViewDimension::D2,
            },
        },
    ],
};

/// Create chunk bind group
fn create_chunk_bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, texture_atlas_view: &wgpu::TextureView, uniform_view_proj: &wgpu::Buffer) -> wgpu::BindGroup {
    // Create texture sampler
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 5.0,
        compare_function: wgpu::CompareFunction::Always,
    });

    device.create_bind_group( &wgpu::BindGroupDescriptor {
        layout,
        bindings: &[
            wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: uniform_view_proj,
                    range: 0..64,
                },
            },
            wgpu::Binding {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::Binding {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(texture_atlas_view),
            },
        ],
    })
}

/*========== SKYBOX RENDERING ==========*/
/// Skybox vertex
#[derive(Debug, Clone, Copy)]
pub struct SkyboxVertex {
    pub position: [f32; 3],
}

/// Skybox vertex attributes
const SKYBOX_VERTEX_ATTRIBUTES: [wgpu::VertexAttributeDescriptor; 1] = [
    wgpu::VertexAttributeDescriptor {
        shader_location: 0,
        format: wgpu::VertexFormat::Float3,
        offset: 0,
    },
];

const SKYBOX_BIND_GROUP_LAYOUT: wgpu::BindGroupLayoutDescriptor<'static> = wgpu::BindGroupLayoutDescriptor {
    bindings: &[
        wgpu::BindGroupLayoutBinding { // view proj
            binding: 0,
            visibility: wgpu::ShaderStage::VERTEX,
            ty: wgpu::BindingType::UniformBuffer { dynamic: false },
        },
        wgpu::BindGroupLayoutBinding { // model
            binding: 1,
            visibility: wgpu::ShaderStage::VERTEX,
            ty: wgpu::BindingType::UniformBuffer { dynamic: false },
        },
    ],
};

/// Create skybox bind group
fn create_vpm_bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, uniform_view_proj: &wgpu::Buffer, uniform_model: &wgpu::Buffer) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout,
        bindings: &[
            wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: uniform_view_proj,
                    range: 0..64,
                },
            },
            wgpu::Binding {
                binding: 1,
                resource: wgpu::BindingResource::Buffer {
                    buffer: uniform_model,
                    range: 0..64,
                },
            },
        ],
    })
}

/*========== TARGET RENDERING ==========*/
// `SkyboxVertex` is shamelessly stolen to also draw the targeted block

/// Create target vertices for some given face
fn create_target_vertices(face: usize) -> Vec<SkyboxVertex> {
    // TODO: simplify this
    let mut vertices = Vec::new();
    fn vpos(i: i32, j: i32, k: i32, face: usize) -> SkyboxVertex {
        let mut v = [i as f32, j as f32, k as f32];
        for i in 0..3 {
            if i == face / 2 {
                // Move face forward
                v[i] += 0.001 * (if face % 2 == 0 { 1.0 } else { -1.0 });
            } else {
                // Move edges inside the face
                if v[i] == 1.0 {
                    v[i] = 0.999;
                } else {
                    v[i] = 0.001;
                }
            }
        }
        SkyboxVertex { position: v }
    }
    let end_coord = [
        if face == 1 { 1 } else { 2 },
        if face == 3 { 1 } else { 2 },
        if face == 5 { 1 } else { 2 },
    ];
    let start_coord = [
        if face == 0 { 1 } else { 0 },
        if face == 2 { 1 } else { 0 },
        if face == 4 { 1 } else { 0 },
    ];
    for i in start_coord[0]..end_coord[0] {
        for j in start_coord[1]..end_coord[1] {
            for k in start_coord[2]..end_coord[2] {
                let mut id = [i, j, k];
                for i in 0..3 {
                    if id[i] > start_coord[i] {
                        let v1 = vpos(id[0], id[1], id[2], face);
                        id[i] = 0;
                        let v2 = vpos(id[0], id[1], id[2], face);
                        id[i] = 1;
                        vertices.extend([v1, v2].into_iter());
                    }
                }
            }
        }
    }
    vertices
}