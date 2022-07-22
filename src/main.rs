use std::sync::Arc;
use std::time::{Instant, Duration};
use std::{fs, io, path::PathBuf};

use actix_web::{get, web::Data, App, HttpResponse, HttpServer, Responder};
use awc::{Connector, ClientBuilder};
use clap::{Args, Parser, Subcommand};
use futures::StreamExt;
use indicatif::ProgressDrawTarget;

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
            "localhost:8080".to_owned()
        };

        println!("Hosting server on {addr} with body {body}");

        HttpServer::new(move || App::new().app_data(Data::new(body.clone())).service(hello))
            .bind(addr)
            .unwrap()
            .run()
            .await
            .unwrap()
    }
}

#[derive(Args, Debug, Clone)]
struct Client {
    #[clap(short, long, default_value = "1")]
    num: usize,
    addr: String,
    #[clap(long)]
    path: Option<PathBuf>,
    #[clap(long)]
    raw: Option<String>,
    #[clap(short, long, default_value = "1")]
    max_outbound_requests: usize,
    #[clap(short, long)]
    stats: bool,
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
        let bar = Arc::new(indicatif::ProgressBar::new(self.num as u64));
        println!("Client running {} tests", self.num);
        bar.set_draw_target(ProgressDrawTarget::stdout());
        for _ in 0..self.num {
            let bar = bar.clone();
            to_run.push(async move {
                let connector = Connector::new().conn_lifetime(Duration::from_secs(1)).limit(1000);
                let client = ClientBuilder::new().connector(connector).finish();
                let start_time = Instant::now();
                let mut res = client.get(self.addr.clone()).send().await.unwrap();
                let body = res.body().await.unwrap();
                let body = if let Ok(body) = std::str::from_utf8(&body) {
                    body.to_owned()
                } else {
                    base64::encode(body)
                };
                if let Some(to_compare) = self.get_body().unwrap() {
                    assert_eq!(to_compare, body);
                    bar.inc(1);
                } else {
                    println!("{body}")
                }
                let elapsed = start_time.elapsed().as_secs_f64() * 1000.0;
                return elapsed;
            });
        }
        let timestamps = futures::stream::iter(to_run.into_iter())
            .buffer_unordered(self.max_outbound_requests)
            .collect::<Vec<_>>()
            .await;
        let timestamps_matrix = nalgebra::Matrix1xX::from_column_slice(&timestamps);
        let mean = timestamps_matrix.mean();
        let std = timestamps_matrix.variance();
        bar.finish_and_clear();
        if self.stats {
            println!("mean: {mean}");
            println!("std: {std}");
        }
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
