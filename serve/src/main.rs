use std::error::Error;
use std::thread::current;
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    const client: Client = reqwest::Client::new();
    
    const url: &str = "";

    let resp = client
        .post(url)
        .json()
        .await?
        .text()
        .await?;
    println!("{:#?}", resp);
    Ok(())
}