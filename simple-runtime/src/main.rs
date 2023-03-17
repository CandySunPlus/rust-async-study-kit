use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use simple_runtime::executor::Executor;
use simple_runtime::tcp::TcpListener;

fn main() {
    let ex = Executor::new();
    ex.block_on(serve);
}

async fn serve() {
    let mut listener = TcpListener::bind("127.0.0.1:30000").unwrap();
    while let Some(ret) = listener.next().await {
        if let Ok((mut stream, addr)) = ret {
            println!("accept a new connection form {addr} successfully");
            let f = async move {
                let mut buf = [0; 4096];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(n) => {
                            if n == 0 || stream.write_all(&buf[..n]).await.is_err() {
                                println!("client disconnected");
                                return;
                            }
                        }
                        Err(_) => return,
                    }
                }
            };

            Executor::spawn(f);
        }
    }
}
