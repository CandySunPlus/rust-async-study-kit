use async_fs::File;
use async_io::Timer;

use futures::{AsyncRead, AsyncReadExt, FutureExt};
use std::{
    fmt,
    io::Result,
    marker::PhantomPinned,
    mem,
    pin::Pin,
    ptr,
    task::{Context, Poll},
    thread,
    time::{Duration, Instant},
};

fn block_on(f: impl std::future::Future + Sync + Send + 'static) {
    let complete = std::sync::Arc::new(std::sync::Mutex::new(core::sync::atomic::AtomicBool::new(
        false,
    )));
    let ender = complete.clone();
    thread::spawn(|| {
        executor::run(async move {
            f.await;
            ender
                .lock()
                .unwrap()
                .store(true, core::sync::atomic::Ordering::Release);
        });
    });
    while !complete
        .lock()
        .unwrap()
        .load(core::sync::atomic::Ordering::Acquire)
    {}
}

struct SlowRead<R> {
    reader: R,
    sleep: Timer,
}

impl<R> SlowRead<R>
where
    R: Unpin,
{
    fn into_inner(self) -> R {
        self.reader
    }
}

impl<R> AsyncRead for SlowRead<R>
where
    R: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        match self.sleep.poll_unpin(cx) {
            Poll::Ready(_) => {
                self.sleep.set_after(Duration::from_millis(25));
                Pin::new(&mut self.reader).poll_read(cx, buf)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<R> SlowRead<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            sleep: Timer::after(Default::default()),
        }
    }
}

async fn run() -> Result<()> {
    let mut buf = vec![0u8; 128 * 1024];
    let mut sr = SlowRead::new(File::open("/dev/urandom").await?);
    let before = Instant::now();
    sr.read_exact(&mut buf).await?;
    println!("Read {} bytes in {:?}", buf.len(), before.elapsed());

    let before = Instant::now();
    let mut f = sr.into_inner();
    f.read_exact(&mut buf).await?;
    println!("Read {} bytes in {:?}", buf.len(), before.elapsed());
    Ok(())
}

struct Test {
    a: String,
    b: *const String,
    _marker: PhantomPinned,
}

impl fmt::Debug for Test {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let this = unsafe { Pin::new_unchecked(self) };
        write!(
            f,
            "Test {{ a: [{:p}] -> {}, b: [{:p}] -> [{:p}] -> {} }}",
            &self.a,
            this.a(),
            &self.b,
            self.b,
            this.b()
        )
    }
}

impl Test {
    fn new(a: &str) -> Self {
        Self {
            a: a.to_owned(),
            b: ptr::null(),
            _marker: PhantomPinned,
        }
    }

    fn init(self: Pin<&mut Self>) {
        let self_ptr = &self.a as *const String;
        let this = unsafe { self.get_unchecked_mut() };
        this.b = self_ptr;
    }

    fn a(self: Pin<&Self>) -> &str {
        &self.get_ref().a
    }

    fn b(self: Pin<&Self>) -> &str {
        unsafe { &*self.b }
    }
}

fn main() -> Result<()> {
    block_on(run());

    let mut t1 = Test::new("test1");
    let mut t1 = unsafe { Pin::new_unchecked(&mut t1) };
    t1.as_mut().init();

    let mut t2 = Test::new("test2");
    let mut t2 = unsafe { Pin::new_unchecked(&mut t2) };
    t2.as_mut().init();

    println!("BEFORE:");
    println!("t1: {:?}\nt2: {:?}", t1, t2);

    mem::swap(&mut t1, &mut t2);
    // mem::swap(t1.get_mut(), t2.get_mut());

    println!("AFTER:");
    println!("t1: {:?}\nt2: {:?}", t1, t2);

    Ok(())
}
