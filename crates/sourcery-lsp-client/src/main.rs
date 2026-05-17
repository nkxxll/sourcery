use std::path::Path;

use async_lsp::lsp_types::DocumentSymbolResponse;
use sourcery_lsp_client::Server;
use tracing::Level;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    run_gopls_smoke().await;
}

/// just a test for the lsp connection
pub async fn run_gopls_smoke() {
    const TEST_ROOT: &str = "../../toanalyze/go-yaml";
    let mut server = Server::new(TEST_ROOT, "gopls", &["serve"]);
    let mainloop = server.run_main_loop();
    let mut socket = server.socket();
    socket.initialize().await;

    let mut first_socket = socket.clone();
    let first = tokio::spawn(async move {
        let line = 691;
        let character = 11;
        let path = Path::new("yaml.go");
        let file_uri = first_socket.open_document(&path).await;
        let goto_definition_res = first_socket
            .goto_definition(file_uri, line, character)
            .await;
        goto_definition_res.unwrap()
    });

    let mut second_socket = socket.clone();
    let second = tokio::spawn(async move {
        let path = Path::new("node.go");
        let file_uri = second_socket.open_document(&path).await;
        let symbols = second_socket.document_symbols(file_uri).await;
        let non_empty = match symbols {
            DocumentSymbolResponse::Flat(list) => !list.is_empty(),
            DocumentSymbolResponse::Nested(list) => !list.is_empty(),
        };
        non_empty
    });

    let (first, second) = tokio::join!(first, second);
    println!(
        "first returned {} second returned {}!",
        match first.unwrap() {
            async_lsp::lsp_types::GotoDefinitionResponse::Scalar(location) =>
                format!("{:?}", location),
            async_lsp::lsp_types::GotoDefinitionResponse::Array(locations) =>
                format!("{:?}", locations),
            async_lsp::lsp_types::GotoDefinitionResponse::Link(location_links) =>
                format!("{:?}", location_links),
        },
        second.unwrap()
    );
    server.shutdown(mainloop).await;
}
