use super::GLContext;
use glow::HasContext;
use std::marker::PhantomData;
use std::{mem, rc::Rc};

struct GLBuffer {
    id: <GLContext as HasContext>::Buffer,
    context: Rc<glow::Context>,
}

impl Drop for GLBuffer {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_buffer(self.id);
        }
    }
}

impl std::ops::Deref for GLBuffer {
    type Target = <GLContext as HasContext>::Buffer;
    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

pub struct GLArrayBuffer<ArrayMemberType> {
    buffer: GLBuffer,
    _type_marker: PhantomData<ArrayMemberType>,
}

impl<ArrayMemberType> GLArrayBuffer<ArrayMemberType> {
    pub fn new(gl: &Rc<glow::Context>, data: &[ArrayMemberType]) -> Self {
        let buffer_id = unsafe { gl.create_buffer().expect("vertex buffer") };

        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer_id));

            let byte_len = mem::size_of_val(&data[0]) * data.len() / mem::size_of::<u8>();
            let byte_slice = std::slice::from_raw_parts(data.as_ptr() as *const u8, byte_len);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, byte_slice, glow::STATIC_DRAW);
        }

        Self { buffer: GLBuffer { id: buffer_id, context: gl.clone() }, _type_marker: PhantomData }
    }

    pub fn bind(&self, gl: &glow::Context, attribute_location: u32) {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(*self.buffer));

            // TODO #5: generalize GL array buffer size/data_type handling beyond f32
            gl.vertex_attrib_pointer_f32(
                attribute_location,
                (mem::size_of::<ArrayMemberType>() / mem::size_of::<f32>()) as i32,
                glow::FLOAT,
                false,
                0,
                0,
            );
            gl.enable_vertex_attrib_array(attribute_location);
        }
    }
}

pub struct GLIndexBuffer<IndexType> {
    buffer: GLBuffer,
    pub len: i32,
    _vertex_marker: PhantomData<IndexType>,
}

impl<IndexType> GLIndexBuffer<IndexType> {
    pub fn new(gl: &Rc<glow::Context>, data: &[IndexType]) -> Self {
        let buffer_id = unsafe { gl.create_buffer().expect("vertex buffer") };

        unsafe {
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(buffer_id));

            let byte_len = mem::size_of_val(&data[0]) * data.len() / mem::size_of::<u8>();
            let byte_slice = std::slice::from_raw_parts(data.as_ptr() as *const u8, byte_len);
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, byte_slice, glow::STATIC_DRAW);
        }

        Self {
            buffer: GLBuffer { id: buffer_id, context: gl.clone() },
            len: data.len() as i32,
            _vertex_marker: PhantomData,
        }
    }

    pub fn bind(&self, gl: &glow::Context) {
        unsafe {
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(*self.buffer));
        }
    }
}
