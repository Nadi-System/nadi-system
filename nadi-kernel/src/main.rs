use anyhow::{Context as _, Result};
use clap::Parser;
use futures::StreamExt;
use jupyter_protocol::{
    ClearOutput, CodeMirrorMode, CommInfoReply, CompleteReply, CompleteRequest, ConnectionInfo,
    DisplayData, ErrorOutput, ExecuteReply, ExecutionCount, HelpLink, HistoryReply, InspectReply,
    IsCompleteReply, IsCompleteReplyStatus, JupyterMessage, JupyterMessageContent, KernelInfoReply,
    LanguageInfo, Media, MediaType, ReplyStatus, ShutdownReply, Status, StreamContent,
};
use nadi_core::{
    attrs::AttrMap,
    parser::{tasks, tokenizer::get_tokens},
    tasks::{Task, TaskContext, TaskMessage},
    template::Template,
};
use runtimelib::{KernelIoPubConnection, RouterRecvConnection, RouterSendConnection};
use serde_json::{Value, json};
use std::{collections::HashMap, env::current_exe};
use uuid::Uuid;

use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the connection file
    #[arg(short, long)]
    connection_file: Option<String>,

    /// Install the kernel
    #[arg(long)]
    install: bool,
}

struct NadiKernel {
    pub context: TaskContext,
    locals: AttrMap,
    pub receiver: Receiver<TaskMessage>,
    pub execution_count: ExecutionCount,
    iopub: KernelIoPubConnection,
    shell: RouterSendConnection,
}

impl NadiKernel {
    pub async fn start(connection_info: &ConnectionInfo) -> Result<()> {
        let session_id = Uuid::new_v4().to_string();

        let mut heartbeat = runtimelib::create_kernel_heartbeat_connection(connection_info).await?;
        let shell_connection =
            runtimelib::create_kernel_shell_connection(connection_info, &session_id).await?;
        let (shell_writer, mut shell_reader) = shell_connection.split();
        let mut control_connection =
            runtimelib::create_kernel_control_connection(connection_info, &session_id).await?;
        let _stdin_connection =
            runtimelib::create_kernel_stdin_connection(connection_info, &session_id).await?;
        let iopub_connection =
            runtimelib::create_kernel_iopub_connection(connection_info, &session_id).await?;

        let (sender, receiver) = channel();
        let context = TaskContext::new(None, sender);

        let mut nadi_kernel = Self {
            context,
            locals: AttrMap::new(),
            receiver,
            execution_count: Default::default(),
            iopub: iopub_connection,
            shell: shell_writer,
        };

        let heartbeat_handle = tokio::spawn({
            async move { while let Ok(()) = heartbeat.single_heartbeat().await {} }
        });

        let control_handle = tokio::spawn({
            async move {
                while let Ok(message) = control_connection.read().await {
                    match &message.content {
                        JupyterMessageContent::KernelInfoRequest(_) => {
                            let sent = control_connection
                                .send(Self::kernel_info().as_child_of(&message))
                                .await;
                            if let Err(err) = sent {
                                eprintln!("Error on control {}", err);
                            }
                        }
                        JupyterMessageContent::ShutdownRequest(req) => {
                            let reply: JupyterMessage = ShutdownReply {
                                restart: req.restart,
                                status: ReplyStatus::Ok,
                                error: None,
                            }
                            .as_child_of(&message);
                            let _ = control_connection.send(reply).await;
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                }
            }
        });

        let shell_handle = tokio::spawn(async move {
            if let Err(err) = nadi_kernel.handle_shell(&mut shell_reader).await {
                eprintln!("Shell error: {}\nBacktrace:\n{}", err, err.backtrace());
            }
        });

        let join_fut =
            futures::future::try_join_all(vec![heartbeat_handle, control_handle, shell_handle]);

        join_fut.await?;

        Ok(())
    }

    async fn clear_output_after_next_output(
        &mut self,
        parent: &JupyterMessage,
    ) -> anyhow::Result<()> {
        Ok(self
            .iopub
            .send(ClearOutput { wait: true }.as_child_of(parent))
            .await?)
    }

    async fn send_image(
        &mut self,
        image_path: &str,
        parent: &JupyterMessage,
    ) -> anyhow::Result<()> {
        Ok(self
            .iopub
            .send(
                DisplayData::from(MediaType::Svg(std::fs::read_to_string(image_path)?))
                    .as_child_of(parent),
            )
            .await?)
    }

    async fn send_markdown(
        &mut self,
        markdown: &str,
        parent: &JupyterMessage,
    ) -> anyhow::Result<()> {
        Ok(self
            .iopub
            .send(DisplayData::from(MediaType::Markdown(markdown.to_string())).as_child_of(parent))
            .await?)
    }

    async fn send_json(
        &mut self,
        json_object: Value,
        parent: &JupyterMessage,
    ) -> anyhow::Result<()> {
        let json_object = match json_object {
            Value::Object(obj) => Value::Object(obj),
            _ => {
                let mut map = serde_json::Map::new();
                map.insert("value".to_string(), json_object);
                Value::Object(map)
            }
        };

        Ok(self
            .iopub
            .send(DisplayData::from(MediaType::Json(json_object)).as_child_of(parent))
            .await?)
    }

    async fn send_error(
        &mut self,
        ename: &str,
        evalue: &str,
        parent: &JupyterMessage,
    ) -> anyhow::Result<()> {
        Ok(self
            .iopub
            .send(
                ErrorOutput {
                    ename: ename.to_string(),
                    evalue: evalue.to_string(),
                    traceback: Default::default(),
                }
                .as_child_of(parent),
            )
            .await?)
    }

    async fn send_info(&mut self, text: &str, parent: &JupyterMessage) -> anyhow::Result<()> {
        Ok(self
            .iopub
            .send(StreamContent::stdout(text).as_child_of(parent))
            .await?)
    }

    async fn send_warning(&mut self, text: &str, parent: &JupyterMessage) -> anyhow::Result<()> {
        Ok(self
            .iopub
            .send(StreamContent::stderr(text).as_child_of(parent))
            .await?)
    }

    async fn push_stdout(&mut self, text: &str, parent: &JupyterMessage) -> anyhow::Result<()> {
        Ok(self
            .iopub
            .send(StreamContent::stdout(text).as_child_of(parent))
            .await?)
    }

    pub async fn handle_shell(&mut self, reader: &mut RouterRecvConnection) -> Result<()> {
        loop {
            let msg = reader.read().await?;
            match self.handle_shell_message(&msg).await {
                Ok(_) => {}
                Err(err) => eprintln!("Error on shell: {}", err),
            }
        }
    }

    async fn complete(&mut self, request: &CompleteRequest) -> anyhow::Result<CompleteReply> {
        let cursor_pos = request.cursor_pos;

        let reply = CompleteReply {
            matches: vec!["node".to_string()],
            cursor_start: cursor_pos,
            cursor_end: cursor_pos,
            metadata: Default::default(),
            status: jupyter_protocol::ReplyStatus::Ok,
            error: None,
        };

        anyhow::Ok(reply)
    }

    async fn execute(&mut self, request: &JupyterMessage) -> anyhow::Result<()> {
        let code = match &request.content {
            JupyterMessageContent::ExecuteRequest(req) => req.code.clone(),
            _ => return Err(anyhow::anyhow!("Invalid message type for execution")),
        };
        let tokens = nadi_core::parser::tokenizer::get_tokens(&code);
        let tasks = match nadi_core::parser::tasks::parse(tokens) {
            Ok(t) => t,
            Err(e) => return Err(anyhow::Error::msg(e.user_msg_color(None))),
        };

        for fc in tasks {
            match self.context.execute(fc, &mut self.locals) {
                Ok(Some(p)) => self.push_stdout(&format!("{p}\n"), request).await?,
                Err(p) => {
                    let msg = {
                        let node = p
                            .node
                            .as_ref()
                            .map(|n| format!("[{n}]"))
                            .unwrap_or_default();
                        if let Some(pos) = p.position.iter().last() {
                            format!(
                                "{node} at Line {} Column {}: {}",
                                pos.0,
                                pos.1,
                                p.ty.message()
                            )
                        } else {
                            format!("{node}: {}", p.ty.message())
                        }
                    };
                    self.send_error(p.ty.name(), &msg, request).await?;
                    break;
                }
                _ => (),
            }
            let messages: Vec<_> = self.receiver.try_iter().collect();
            for msg in messages {
                match msg {
                    TaskMessage::Image(img) => self.send_image(&img, request).await?,
                    TaskMessage::Info(txt) => self.send_info(&txt, request).await?,
                    TaskMessage::Warning(txt) => self.send_warning(&txt, request).await?,
                    _ => (),
                }
            }
        }
        Ok(())
    }

    pub async fn handle_shell_message(&mut self, parent: &JupyterMessage) -> Result<()> {
        // Even with messages like `kernel_info_request`, you're required to send a busy and idle message
        self.iopub.send(Status::busy().as_child_of(parent)).await?;

        match &parent.content {
            JupyterMessageContent::CommInfoRequest(_) => {
                // Just tell the frontend we don't have any comms
                let reply = CommInfoReply {
                    status: ReplyStatus::Ok,
                    comms: Default::default(),
                    error: None,
                }
                .as_child_of(parent);
                self.shell.send(reply).await?;
            }
            JupyterMessageContent::CompleteRequest(req) => {
                let reply = self.complete(req).await?;
                self.shell.send(reply.as_child_of(parent)).await?;
            }
            JupyterMessageContent::ExecuteRequest(_) => {
                // Respond back with reply immediately
                let reply = ExecuteReply {
                    status: ReplyStatus::Ok,
                    execution_count: self.one_up_execution_count(),
                    user_expressions: Default::default(),
                    payload: Default::default(),
                    error: None,
                }
                .as_child_of(parent);
                self.shell.send(reply).await?;

                if let Err(err) = self.execute(parent).await {
                    self.send_error("NadiFailure", &err.to_string(), parent)
                        .await?;
                }
            }
            JupyterMessageContent::HistoryRequest(_) => {
                let reply = HistoryReply {
                    history: Default::default(),
                    status: ReplyStatus::Ok,
                    error: None,
                }
                .as_child_of(parent);
                self.shell.send(reply).await?;
            }
            JupyterMessageContent::InspectRequest(_) => {
                // Would be really cool to have the model inspect at the word,
                // kind of like an editor.

                let reply = InspectReply {
                    found: false,
                    data: Media::default(),
                    metadata: Default::default(),
                    status: ReplyStatus::Ok,
                    error: None,
                }
                .as_child_of(parent);

                self.shell.send(reply).await?;
            }
            JupyterMessageContent::IsCompleteRequest(_) => {
                // true, unconditionally
                let reply = IsCompleteReply {
                    status: IsCompleteReplyStatus::Complete,
                    indent: "".to_string(),
                }
                .as_child_of(parent);

                self.shell.send(reply).await?;
            }
            JupyterMessageContent::KernelInfoRequest(_) => {
                let reply = Self::kernel_info().as_child_of(parent);

                self.shell.send(reply).await?;
            }
            // Not implemented for shell includes DebugRequest
            // Not implemented for control (and sometimes shell...) includes InterruptRequest, ShutdownRequest
            _ => {}
        };

        self.iopub.send(Status::idle().as_child_of(parent)).await?;

        Ok(())
    }

    fn kernel_info() -> KernelInfoReply {
        KernelInfoReply {
            status: ReplyStatus::Ok,
            protocol_version: "5.3".to_string(),
            implementation: "Nadi Kernel".to_string(),
            implementation_version: "0.1".to_string(),
            language_info: LanguageInfo {
                name: "tasks".to_string(),
                version: "0.1".to_string(),
                mimetype: Some("text/nadi".to_string()),
                file_extension: Some(".tasks".to_string()),
                pygments_lexer: None,
                codemirror_mode: None,
                nbconvert_exporter: Some("script".to_string()),
            },
            banner: "Nadi Kernel".to_string(),
            help_links: vec![
                HelpLink {
                    text: "NADI Website".to_string(),
                    url: "https://nadi-system.github.io/".to_string(),
                },
                HelpLink {
                    text: "NADI User Guide".to_string(),
                    url: "https://nadi-system.github.io/0.8.0/".to_string(),
                },
            ],
            debugger: false,
            error: None,
        }
    }

    fn one_up_execution_count(&mut self) -> ExecutionCount {
        self.execution_count.0 += 1;
        self.execution_count
    }
}

pub async fn start_kernel(connection_filepath: &str) -> anyhow::Result<()> {
    let conn_file = std::fs::read_to_string(connection_filepath)
        .with_context(|| format!("Couldn't read connection file: {:?}", connection_filepath))?;
    let spec: ConnectionInfo = serde_json::from_str(&conn_file).with_context(|| {
        format!(
            "Connection file is not a valid JSON: {:?}",
            connection_filepath
        )
    })?;

    println!("Starting Nadi Kernel");
    NadiKernel::start(&spec).await?;

    anyhow::Ok(())
}

async fn install_kernel() -> anyhow::Result<()> {
    println!("Installing NADI Kernel...");
    let user_data_dir = runtimelib::user_data_dir()?;
    let kernel_dir = user_data_dir.join("kernels").join("nadi");
    tokio::fs::create_dir_all(&kernel_dir).await?;
    let kernel_json_path = kernel_dir.join("kernel.json");
    let json_data = json!({
        "argv": [current_exe()?.to_string_lossy(), "--connection-file", "{connection_file}"],
        "display_name": "NADI",
        "language": "tasks",
    });
    let mut f = tokio::fs::File::create(kernel_json_path).await?;
    tokio::io::AsyncWriteExt::write_all(
        &mut f,
        serde_json::to_string_pretty(&json_data)?.as_bytes(),
    )
    .await?;
    println!("NADI Kernel installed successfully!");
    // todo: Include icons during installation
    anyhow::Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.install {
        install_kernel().await?;
    } else if let Some(connection_filepath) = args.connection_file {
        start_kernel(&connection_filepath).await?;
    } else {
        eprintln!("Error: Either --install or --connection-file must be provided");
        std::process::exit(1);
    }

    Ok(())
}
