use std::io;

use bitcode::{Decode, Encode};
use tokio_util::{
    bytes::Bytes,
    codec::{Decoder, Encoder, LengthDelimitedCodec},
};

#[derive(Debug, Encode, Decode)]
pub struct DrawMessage {
    pub name: String,
    pub shapes: Vec<Shape>,
}

#[derive(Debug, Encode, Decode)]
pub enum Shape {
    Circle(CircleShape),
    Rect(RectShape),
    Segment(SegmentShape),
}

#[derive(Debug, Encode, Decode)]
pub struct CircleShape {
    pub pos: (f32, f32),
    pub radius: f32,
}

#[derive(Debug, Encode, Decode)]
pub struct RectShape {
    pub top_left: (f32, f32),
    pub size: (f32, f32),
    pub rotate: f32,
}

#[derive(Debug, Encode, Decode)]
pub struct SegmentShape {
    pub p1: (f32, f32),
    pub p2: (f32, f32),
}

#[derive(Debug, thiserror::Error)]
pub enum ShapeCodecError {
    #[error("io Error, inner: {0}")]
    Io(io::Error),
    #[error("bitcode Error, inner: {0}")]
    BitCode(bitcode::Error),
}

impl From<io::Error> for ShapeCodecError {
    fn from(value: io::Error) -> Self {
        ShapeCodecError::Io(value)
    }
}

impl From<bitcode::Error> for ShapeCodecError {
    fn from(value: bitcode::Error) -> Self {
        ShapeCodecError::BitCode(value)
    }
}

pub struct ShapeCodec {
    len_delimited_codec: LengthDelimitedCodec,
    bitcode_buffer: bitcode::Buffer,
}

impl ShapeCodec {
    pub fn new() -> Self {
        let len_delimited_codec = LengthDelimitedCodec::builder()
            .max_frame_length(32 * 1_024 * 1_024)
            .new_codec();

        ShapeCodec {
            len_delimited_codec,
            bitcode_buffer: bitcode::Buffer::new(),
        }
    }
}

impl Decoder for ShapeCodec {
    type Item = DrawMessage;

    type Error = ShapeCodecError;

    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        let Some(encoded_shpes) = self.len_delimited_codec.decode(src)? else {
            return Ok(None);
        };
        Ok(Some(self.bitcode_buffer.decode(&encoded_shpes)?))
    }
}

impl Encoder<DrawMessage> for ShapeCodec {
    type Error = ShapeCodecError;

    fn encode(
        &mut self,
        item: DrawMessage,
        dst: &mut tokio_util::bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        let bytes = self.bitcode_buffer.encode(&item);
        self.len_delimited_codec
            .encode(Bytes::copy_from_slice(bytes), dst)
            .map_err(Into::into)
    }
}
