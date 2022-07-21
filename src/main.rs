use std::{path::PathBuf, fs, io};

use actix_web::{get, App, HttpResponse, HttpServer, Responder, web::Data};
use clap::{Parser, Args, Subcommand};

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    run_type: RunType,
}

#[derive(Subcommand, Debug, Clone)]
enum RunType {
    Server(Server),
    Client(Client),
}

#[derive(Args, Debug, Clone)]
struct Server {
    #[clap(long)]
    addr: Option<String>,
    #[clap(long)]
    path: Option<PathBuf>,
    #[clap(long)]
    raw: Option<String>,   
}

impl Server {
    pub fn get_body(&self) -> Result<String, io::Error> {
        if let Some(path) = &self.path {
            fs::read_to_string(&path)
        } else if let Some(raw) = &self.raw {
            Ok(raw.to_owned())
        } else {
            Ok(String::new())
        }
    }

    pub async fn run(&self) {
        let body = self.get_body().unwrap();
        let addr = if let Some(addr) = &self.addr {
            addr.to_owned()
        } else {
            "127.0.0.1:8080".to_owned()
        };

        println!("Hosting server on {addr} with body {body}");

        HttpServer::new(move || {
            App::new()
                .app_data(Data::new(body.clone()))
                .service(hello)
        })
        .bind(addr)
        .unwrap()
        .run()
        .await.unwrap()
    }
}

#[derive(Args, Debug, Clone)]
struct Client {
    #[clap(short, long, default_value="1")]
    num: usize,
    addr: String,
    #[clap(long)]
    path: Option<PathBuf>,
    #[clap(long)]
    raw: Option<String>,
}

impl Client {
    pub fn get_body(&self) -> Result<Option<String>, io::Error> {
        if let Some(path) = &self.path {
            fs::read_to_string(&path).map(|str| Some(str))
        } else if let Some(raw) = &self.raw {
            Ok(Some(raw.to_owned()))
        } else {
            Ok(None)
        }
    }
    
    pub async fn run(&self) {
        let mut to_run = Vec::new();
        for _ in 0..self.num {
            to_run.push(async {
                let client = awc::Client::default();
                let mut res = client.get(self.addr.clone()).send().await.unwrap();
                let body = res.body().await.unwrap();
                let body = if let Ok(body) = std::str::from_utf8(&body) {
                    body.to_owned()
                } else {
                    base64::encode(body)
                };
                if let Some(to_compare) = self.get_body().unwrap() {
                    assert_eq!(to_compare, body)
                } else {
                    println!("{body}")
                }
            });
        }
        futures::future::join_all(to_run.into_iter()).await;
    }
}

#[get("/")]
async fn hello(data: Data<String>) -> impl Responder {
    HttpResponse::Ok().body(data.as_ref().to_owned())
}

#[actix_web::main]
async fn main() {
    let cli = Cli::parse();
    match cli.run_type {
        RunType::Server(server) => server.run().await,
        RunType::Client(client) => client.run().await,
    }
}
