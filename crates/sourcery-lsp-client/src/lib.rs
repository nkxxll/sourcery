use std::fmt::Display;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Result, anyhow};
use async_lsp::concurrency::{Concurrency, ConcurrencyLayer};
use async_lsp::lsp_types::notification::{LogMessage, Progress, PublishDiagnostics, ShowMessage};
use async_lsp::lsp_types::{
    self, ClientCapabilities, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse,
    InitializeParams, InitializedParams, Location, PartialResultParams, ReferenceContext,
    ReferenceParams, TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Url,
    WindowClientCapabilities, WorkDoneProgressParams, WorkspaceFolder,
};
use async_lsp::panic::{CatchUnwind, CatchUnwindLayer};
use async_lsp::router::Router;
use async_lsp::tracing::{Tracing, TracingLayer};
use async_lsp::{Error, ErrorCode, LanguageServer, MainLoop, ServerSocket};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tower::ServiceBuilder;
use tracing::{debug, info, warn};

struct Stop;
type InnerMainLoop = Tracing<CatchUnwind<Concurrency<Router<()>>>>;
type OptionReferences = Option<Vec<Location>>;

/// public range type from the lsp
#[derive(Debug)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Range")
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

impl From<Range> for lsp_types::Range {
    fn from(value: Range) -> Self {
        lsp_types::Range {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

impl From<lsp_types::Range> for Range {
    fn from(value: lsp_types::Range) -> Self {
        Range {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

/// own lsp position implementation to be able to publish it to the analyzer
#[derive(Debug)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl From<lsp_types::Position> for Position {
    fn from(pos: lsp_types::Position) -> Self {
        Self {
            line: pos.line,
            character: pos.character,
        }
    }
}

impl From<Position> for lsp_types::Position {
    fn from(pos: Position) -> Self {
        Self {
            line: pos.line,
            character: pos.character,
        }
    }
}

/// this holds all the data that has to be shared (like channels) with the threads that make requests to the server thread
#[derive(Clone)]
pub struct SharedSocket {
    socket: ServerSocket,
    root_dir: PathBuf,
}

/// this holds all the informtion that needs to be owned by the server process
/// itself so this cannot be cloned and this is ok
pub struct Server {
    mainloop: Option<MainLoop<InnerMainLoop>>,
    socket: SharedSocket,
    child: Child,
}

impl Server {
    pub fn new(root_dir: impl AsRef<Path>, lsp_binary_name: &str, server_args: &[&str]) -> Self {
        let (mainloop, socket) = Self::setup_ls();
        let root_dir = root_dir
            .as_ref()
            .canonicalize()
            .expect("test root should be valid");

        let child = Command::new(lsp_binary_name)
            .args(server_args)
            .current_dir(&root_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .expect("failed to start language server");

        Self {
            mainloop: Some(mainloop),
            socket: SharedSocket { socket, root_dir },
            child,
        }
    }

    pub async fn initialize(&mut self) {
        self.socket.initialize().await;
    }

    pub fn socket(&self) -> SharedSocket {
        self.socket.clone()
    }

    fn setup_ls() -> (MainLoop<InnerMainLoop>, ServerSocket) {
        async_lsp::MainLoop::new_client(|_server| {
            let mut router = Router::new(());
            router
                .notification::<Progress>(|_, prog| {
                    tracing::debug!("{:?} {:?}", prog.token, prog.value);
                    ControlFlow::Continue(())
                })
                .notification::<PublishDiagnostics>(|_, _| ControlFlow::Continue(()))
                .notification::<ShowMessage>(|_, params| {
                    tracing::debug!("Message {:?}: {}", params.typ, params.message);
                    ControlFlow::Continue(())
                })
                .notification::<LogMessage>(|_, params| {
                    tracing::debug!("Log {:?}: {}", params.typ, params.message);
                    ControlFlow::Continue(())
                })
                .event(|_, _: Stop| ControlFlow::Break(Ok(())));

            ServiceBuilder::new()
                .layer(TracingLayer::default())
                .layer(CatchUnwindLayer::default())
                .layer(ConcurrencyLayer::default())
                .service(router)
        })
    }

    pub fn run_main_loop(&mut self) -> JoinHandle<()> {
        info!("starting language server main loop");
        let stdout = self.child.stdout.take().expect("missing server stdout");
        let stdin = self.child.stdin.take().expect("missing server stdin");
        let mainloop = self
            .mainloop
            .take()
            .expect("mainloop already started and moved");

        tokio::spawn(async move {
            mainloop
                .run_buffered(stdout.compat(), stdin.compat_write())
                .await
                .unwrap();
            info!("language server main loop stopped");
        })
    }

    /// shutdown gracefully
    ///
    /// first shutdown
    /// then exit
    /// then send stop
    /// then wait for the main loop to finished
    /// this is all done so that notifications and messages still in flight can
    /// be delivered and the server can shutdown with return 0
    /// the timeout just makes sure that it will be shutdown eventually even if
    /// this means that it will be shutdown forcfully after 2 seconds
    pub async fn shutdown(&mut self, mainloop: JoinHandle<()>) {
        info!("starting language server shutdown");
        let mut socket = self.socket.socket.clone();
        socket.shutdown(()).await.unwrap();
        socket.exit(()).unwrap();
        socket.emit(Stop).unwrap();
        mainloop.await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await;
        info!("finished language server shutdown");
    }
}

impl SharedSocket {
    pub async fn initialize(&mut self) {
        info!(root = %self.root_dir.display(), "initializing language server");
        let init_ret = self
            .socket
            .initialize(InitializeParams {
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: Url::from_file_path(&self.root_dir)
                        .expect("root_dir should be a valid file URL"),
                    name: "root".into(),
                }]),
                capabilities: ClientCapabilities {
                    window: Some(WindowClientCapabilities {
                        work_done_progress: Some(true),
                        ..WindowClientCapabilities::default()
                    }),
                    ..ClientCapabilities::default()
                },
                ..InitializeParams::default()
            })
            .await
            .unwrap();
        info!("Initialized: {init_ret:?}");
        self.socket.initialized(InitializedParams {}).unwrap();
        info!("sent initialized notification");
    }

    pub async fn close_document(&mut self, path: &Path) {
        debug!(file = %path.display(), "sending did_close");
        let file_uri = Url::from_file_path(path).unwrap();
        self.socket
            .did_close(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: file_uri },
            })
            .unwrap();
        debug!(file = %path.display(), "sent did_close");
    }

    pub fn project_path_to_uri(path: &Path) -> Result<Url> {
        let path = path.canonicalize()?;
        match Url::from_file_path(&path) {
            Ok(url) => Ok(url),
            Err(_) => Err(anyhow!(
                "error converting path to uri {}",
                &path.to_string_lossy()
            )),
        }
    }

    pub async fn open_document(&mut self, path: &Path) -> Url {
        debug!(file = %path.display(), "opening document for language server");
        let file_uri = Url::from_file_path(
            path.canonicalize()
                .expect("file could not be canoncialized in open document what the duc"),
        )
        .unwrap();
        let text = tokio::fs::read_to_string(path)
            .await
            .expect("failed to read file");

        self.socket
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: file_uri.clone(),
                    language_id: "go".into(),
                    version: 1,
                    text,
                },
            })
            .unwrap();
        debug!(file = %path.display(), "sent did_open");
        file_uri
    }

    pub async fn find_references(
        &mut self,
        uri: Url,
        position: Position,
    ) -> Result<OptionReferences> {
        debug!(
            uri = %uri,
            line = position.line,
            character = position.character,
            "sending textDocument/references request"
        );
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position.into(),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration: false,
            },
        };
        let res = self.socket.references(params).await;
        match res {
            Ok(r) => {
                debug!(
                    uri = %uri,
                    references = r.as_ref().map_or(0, Vec::len),
                    "received textDocument/references response"
                );
                Ok(r)
            }
            Err(e) => Err(anyhow!("error finding references e: {}", e)),
        }
    }

    pub async fn goto_definition(&mut self, uri: Url, position: Position) -> Result<Vec<Location>> {
        debug!(
            uri = %uri,
            line = position.line,
            character = position.character,
            "sending textDocument/definition request"
        );
        let goto_definition_params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position.into(),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        let ret = self.socket.definition(goto_definition_params).await;
        match ret {
            Ok(res) => {
                if let Some(res) = res {
                    match res {
                        GotoDefinitionResponse::Scalar(location) => {
                            debug!(uri = %uri, definitions = 1, "received scalar definition response");
                            Ok(vec![location])
                        }
                        GotoDefinitionResponse::Array(locations) => {
                            debug!(
                                uri = %uri,
                                definitions = locations.len(),
                                "received array definition response"
                            );
                            Ok(locations)
                        }
                        GotoDefinitionResponse::Link(_location_links) => {
                            unreachable!("we shouldn't enalbe link in th capabilities")
                        }
                    }
                } else {
                    warn!(uri=%uri , "there was no response found for gotodefinition");
                    Ok(vec![])
                }
            }
            Err(err) => Err(anyhow!(
                "Error while getting goto data from function {}",
                err
            )),
        }
    }

    /// todo: the error handling is horrible here
    pub async fn document_symbols(&mut self, file_uri: Url) -> DocumentSymbolResponse {
        debug!(uri = %file_uri, "starting document symbols request loop");
        let mut retry_count = 0u32;
        loop {
            let ret = self
                .socket
                .document_symbol(DocumentSymbolParams {
                    text_document: TextDocumentIdentifier {
                        uri: file_uri.clone(),
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                })
                .await;

            match ret {
                Ok(resp) => {
                    debug!(
                        uri = %file_uri,
                        retries = retry_count,
                        "received document symbols response"
                    );
                    return resp.expect("no document symbols");
                }
                Err(Error::Response(resp)) if resp.code == ErrorCode::CONTENT_MODIFIED => {
                    retry_count = retry_count.saturating_add(1);
                    warn!(
                        uri = %file_uri,
                        retries = retry_count,
                        "document symbols content modified, retrying"
                    );
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                Err(err) => panic!("request failed: {err}"),
            }
        }
    }
}
