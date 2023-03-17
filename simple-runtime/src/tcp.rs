use std::cell::RefCell;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{
    Shutdown, SocketAddr, TcpListener as StdTcpListener, TcpStream as StdTcpStream, ToSocketAddrs,
};
use std::os::fd::AsRawFd;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll};

use futures::{AsyncRead, AsyncWrite, Stream};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::reactor::{get_reactor, Reactor};

pub struct TcpListener {
    listener: StdTcpListener,
    reactor: Weak<RefCell<Reactor>>,
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self, io::Error> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "empty address"))?;

        let domain = if addr.is_ipv6() {
            Domain::IPV6
        } else {
            Domain::IPV4
        };

        let sk = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;

        let addr = SockAddr::from(addr);

        sk.set_reuse_address(true)?;
        sk.bind(&addr)?;
        sk.listen(1024)?;

        let reactor = get_reactor();
        reactor.borrow_mut().add(sk.as_raw_fd());

        Ok(Self {
            listener: sk.into(),
            reactor: Rc::downgrade(&reactor),
        })
    }
}

impl Stream for TcpListener {
    type Item = io::Result<(TcpStream, SocketAddr)>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.listener.accept() {
            Ok((stream, addr)) => Poll::Ready(Some(Ok((stream.into(), addr)))),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                let reactor = self.reactor.upgrade().unwrap();
                reactor
                    .borrow_mut()
                    .interest_readable(self.listener.as_raw_fd(), cx);
                Poll::Pending
            }
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }
}

pub struct TcpStream {
    stream: StdTcpStream,
}

impl From<StdTcpStream> for TcpStream {
    fn from(value: StdTcpStream) -> Self {
        let reactor = get_reactor();
        reactor.borrow_mut().add(value.as_raw_fd());
        Self { stream: value }
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let reactor = get_reactor();
        reactor.borrow_mut().delete(self.stream.as_raw_fd());
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.stream.read(buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                let reactor = get_reactor();
                reactor
                    .borrow_mut()
                    .interest_readable(self.stream.as_raw_fd(), cx);
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.stream.write(buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                let reactor = get_reactor();
                reactor
                    .borrow_mut()
                    .interest_writable(self.stream.as_raw_fd(), cx);
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.stream.shutdown(Shutdown::Write)?;
        Poll::Ready(Ok(()))
    }
}
