use async_io::Async;
use std::{
    cell::RefCell,
    io::{self, Write},
    os::windows::io::{AsRawSocket, AsSocket, BorrowedSocket, RawSocket},
    path::Path,
    sync::OnceLock,
};
use tokio::sync::mpsc::{Sender, channel};
use tokio_util::{bytes::BytesMut, codec::Encoder};

use shared_types::{CircleShape, DrawMessage, RectShape, Shape, ShapeCodec};

#[cxx::bridge(namespace = "rs::box2d_viewer")]
mod ffi {
    extern "Rust" {
        pub fn draw_remote(sock: &str, name: &str, circles: &[f32], rects: &[f32]);
    }
}

pub fn draw_remote(sock: &str, name: &str, circles: &[f32], rects: &[f32]) {
    thread_local! {
        static ENCODER: RefCell<ShapeCodec> = RefCell::new(ShapeCodec::new());
    }

    static TX: OnceLock<Option<Sender<BytesMut>>> = OnceLock::new();

    let init_tx = || {
        let (tx, mut rx) = channel::<BytesMut>(10);
        let sock = sock.to_owned();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            rt.block_on(async {
                let mut stream: Option<Async<UnixStream>> = None;
                while let Some(data) = rx.recv().await {
                    if stream.is_none() {
                        let Ok(it) = UnixStream::connect(&sock) else {
                            continue;
                        };
                        let Ok(it) = Async::new(it) else {
                            continue;
                        };

                        stream = Some(it);
                    }

                    if let Some(ref mut ss) = stream {
                        if let Err(_) =
                            unsafe { ss.write_with_mut(|it| it.write_all(&data[..])) }.await
                        {
                            // need reconnect
                            stream.take();
                        }
                    }
                }
            });
        });

        Some(tx)
    };

    let tx = TX.get_or_init(init_tx);
    if let Some(tx) = tx {
        let circles = circles.chunks_exact(3).map(|it| {
            Shape::Circle(CircleShape {
                pos: (it[0], it[1]),
                radius: it[2],
            })
        });
        let rects = rects.chunks_exact(5).map(|it| {
            Shape::Rect(RectShape {
                top_left: (it[0], it[1]),
                size: (it[2], it[3]),
                rotate: it[4],
            })
        });

        if tx.try_reserve().is_ok() {
            let msg = DrawMessage {
                name: name.to_owned(),
                shapes: circles.chain(rects).collect(),
            };

            let mut data = BytesMut::new();
            ENCODER.with_borrow_mut(|encoder| {
                encoder.encode(msg, &mut data).unwrap();
            });

            let _ = tx.try_send(data);
        }
    }
}

#[derive(Debug)]
struct UnixStream(uds_windows::UnixStream);

impl From<uds_windows::UnixStream> for UnixStream {
    fn from(value: uds_windows::UnixStream) -> Self {
        UnixStream(value)
    }
}

impl AsSocket for UnixStream {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.0.as_socket()
    }
}

impl AsRawSocket for UnixStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

impl Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

unsafe impl async_io::IoSafe for UnixStream {}

impl UnixStream {
    fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        uds_windows::UnixStream::connect(path).map(UnixStream)
    }
}
