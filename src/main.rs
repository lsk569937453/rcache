use anyhow::anyhow;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create a TCP listener bound to the specified address
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind to address"))?;

    println!("Server listening on {}", addr);

    loop {
        let (socket, _) = listener
            .accept()
            .await
            .expect("Failed to accept incoming connection");

        // Spawn a new task to handle each incoming connection
        task::spawn(async move {
            handle_connection(socket).await;
        });
    }

    Ok(())
}

async fn handle_connection(mut socket: TcpStream) {
    // Process the incoming connection
    println!("New client connected");

    let mut buf = vec![0u8; 1024];
    loop {
        match socket.read(&mut buf).await {
            Ok(0) => {
                println!("Connection closed by client");
                break;
            }
            Ok(n) => {
                // Process the received data
                let msg = String::from_utf8_lossy(&buf[..n]);
                println!("Received: {}", msg);

                // Echo the received data back to the client
                if let Err(_) = socket.write_all(&buf[..n]).await {
                    println!("Error writing data to socket");
                    break;
                }
            }
            Err(err) => {
                println!("Error reading data from socket: {}", err);
                break;
            }
        }
    }
}
