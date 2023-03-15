use std::time::Instant;

use executor::block_on;
use future::Task;
use reactor::Reactor;

mod executor;
mod future;
mod reactor;

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

    let main_fut = async {
        fut1.await;
        fut2.await;
    };

    block_on(main_fut);

    reactor.lock().map(|mut r| r.close()).unwrap();
}
