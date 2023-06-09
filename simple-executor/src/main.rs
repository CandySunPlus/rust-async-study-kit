use std::time::{Duration, Instant};

use simple_executor::best_executor;
use simple_executor::executor::new_exector_and_spawner;
use simple_executor::timer_future::TimerFuture;

fn main() {
    let (executor, spawner) = new_exector_and_spawner();

    spawner.spawn(async {
        let start = Instant::now();
        println!("executor start: {:.2}", start.elapsed().as_secs_f32());
        TimerFuture::new(Duration::from_secs(5)).await;
        println!("executor end: {:.2}", start.elapsed().as_secs_f32());
    });

    best_executor::spawn(async {
        let start = Instant::now();
        println!("best executor start: {:.2}", start.elapsed().as_secs_f32());
        TimerFuture::new(Duration::from_secs(5)).await;
        println!("best executor end: {:.2}", start.elapsed().as_secs_f32());
    });

    // if we don't drop the spawner manually, the executor's receiver will keep blocking
    drop(spawner);

    executor.run();
}
