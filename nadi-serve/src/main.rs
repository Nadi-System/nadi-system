use nadi_core::{prelude::*, tasks::Task};
use rocket::Responder;
use rocket::fs::NamedFile;
use rocket::serde::json::Json;
use rocket::{self, get, launch, post, routes};
use serde::Deserialize;
use std::str::FromStr;

#[derive(Deserialize)]
struct ReqData {
    network: Option<String>,
    tasks: String,
}

struct ReqTasks {
    network: Option<Network>,
    tasks: Vec<Task>,
}

#[derive(Responder)]
enum TaskResult {
    #[response(status = 400)]
    ParseError(String),
    #[response(status = 400)]
    EvalError(String),
    #[response(status = 200)]
    Success(String),
}

impl ReqTasks {
    fn blacklist() -> Vec<&'static str> {
        vec!["command"]
    }
    fn new(data: ReqData) -> Result<Self, String> {
        let network = if let Some(net) = &data.network {
            Some(Network::from_str(net).map_err(|e| e.user_msg(None))?)
        } else {
            None
        };
        let tokens = nadi_core::parser::tokenizer::get_tokens(&data.tasks);
        let tasks = match nadi_core::parser::tasks::parse(tokens) {
            Ok(t) => t,
            Err(e) => return Err(e.user_msg(None)),
        };
        Ok(Self { network, tasks })
    }
    fn execute(self) -> TaskResult {
        let mut ctx = nadi_core::tasks::TaskContextWrap::new(self.network);
        for p in Self::blacklist() {
            ctx.context.functions.remove_plugin(p); // security concerns
        }

        let mut results = Vec::with_capacity(self.tasks.len());
        let mut locals = AttrMap::new();
        for fc in self.tasks {
            match ctx.execute(fc, &mut locals) {
                Ok(Some(p)) => results.push(p),
                Err(p) => {
                    return TaskResult::EvalError(format!(
                        "{}\n\nError: {}",
                        results.join("\n"),
                        p
                    ));
                }
                _ => (),
            }
        }
        TaskResult::Success(results.join("\n"))
    }
}

#[get("/")]
async fn index() -> Result<NamedFile, std::io::Error> {
    NamedFile::open("index.html").await
}

#[post("/evaluate", data = "<tasks>")]
fn eval(tasks: Json<ReqData>) -> TaskResult {
    match ReqTasks::new(tasks.into_inner()) {
        Ok(t) => t.execute(),
        Err(e) => TaskResult::ParseError(e),
    }
}

#[launch]
fn launch() -> _ {
    rocket::build()
        .mount("/", routes![index])
        .mount("/", routes![eval])
}
