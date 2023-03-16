use std::time::Instant;

use custom_async::executor::block_on;
use custom_async::future::Task;
use custom_async::reactor::Reactor;
use futures::future::join;

fn main() {
    let start = Instant::now();
    let reactor = Reactor::new();

    let fut1 = async {
        let val = Task::new(reactor.clone(), 1, 1).await;
        println!("Got {val} at time: {:.2}", start.elapsed().as_secs_f32());
    };

    let fut2 = async {
        let val = Task::new(reactor.clone(), 2, 2).await;
        println!("Got {val} at time: {:.2}", start.elapsed().as_secs_f32());
    };

    let main_fut = join(fut2, fut1);

    block_on(main_fut);

    reactor.lock().map(|mut r| r.close()).unwrap();
}
