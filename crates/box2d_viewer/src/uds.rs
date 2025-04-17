use std::{
    io,
    os::windows::io::{AsRawSocket, AsSocket, BorrowedSocket, RawSocket},
    path::Path,
};

use uds_windows::SocketAddr;

pub struct UnixListener(uds_windows::UnixListener);

impl AsRawSocket for UnixListener {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

impl AsSocket for UnixListener {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
    }
}

impl UnixListener {
    pub fn bind(path: impl AsRef<Path>) -> io::Result<Self> {
        uds_windows::UnixListener::bind(path).map(UnixListener)
    }

    pub fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        self.0.accept().map(|it| (UnixStream(it.0), it.1))
    }
}

#[derive(Debug)]
pub struct UnixStream(uds_windows::UnixStream);

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

impl io::Read for UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

unsafe impl async_io::IoSafe for UnixStream {}
