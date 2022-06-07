use std::sync::Arc;

use apalis::layers::{Extension, TraceLayer};
use apalis::redis::RedisStorage;
use apalis::{
    Job, JobContext, JobError, JobResult, Monitor, Storage, WorkerBuilder, WorkerFactoryFn,
};
use futures::future;
use serde::{Deserialize, Serialize};

use actix_web::{web, App, HttpResponse, HttpServer};

#[derive(Debug, Deserialize, Serialize)]
pub struct ForgottenEmail {
    pub email: String,
}

impl Job for ForgottenEmail {
    const NAME: &'static str = "awesome-app::ForgottenEmail";
}

#[derive(Clone)]
struct MailProviderClient {
    // ...
}

async fn send_email(_email: ForgottenEmail, ctx: JobContext) -> Result<JobResult, JobError> {
    let _client: &Arc<MailProviderClient> = ctx.data_opt().unwrap();
    Ok(JobResult::Success)
}

async fn forgotten_email_endpoint(
    email: web::Json<ForgottenEmail>,
    storage: web::Data<RedisStorage<ForgottenEmail>>,
) -> HttpResponse {
    let storage = &*storage.into_inner();
    let mut storage = storage.clone();
    let res = storage.push(email.into_inner()).await;
    match res {
        Ok(()) => HttpResponse::Ok().body(format!("ForgottenEmail added to queue")),
        Err(e) => HttpResponse::InternalServerError().body(format!("{}", e)),
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();
    let redis_url = std::env::var("REDIS_URL").expect("Please include REDIS_URL in env");
    let storage = RedisStorage::connect(redis_url)
        .await
        .expect("Could not connect to redis storage");

    let data = web::Data::new(storage.clone());
    let http = HttpServer::new(move || {
        App::new().app_data(data.clone()).service(
            web::scope("/accounts")
                .route("/forgot-password", web::post().to(forgotten_email_endpoint)),
        )
    })
    .bind("127.0.0.1:8000")?
    .run();
    let client = Arc::new(MailProviderClient {});
    let worker = Monitor::new()
        .register_with_count(2, move |_| {
            WorkerBuilder::new(storage.clone())
                .layer(TraceLayer::new())
                .layer(Extension(client.clone()))
                .build_fn(send_email)
        })
        .run();

    future::try_join(http, worker).await?;
    Ok(())
}
