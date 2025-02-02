use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;
use std::pin::Pin;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio_stream::{Stream, StreamExt};
use crate::settings::SETTINGS;

pub enum TextGeneratorType {
    OllamaGenerator,
    ShellScriptGenerator,
}

impl TextGeneratorType {
    pub(crate) async fn generate(&self, context: String) -> Pin<Box<dyn Stream<Item = String>>> {
        match self {
            TextGeneratorType::OllamaGenerator => {
                let model = { SETTINGS.lock().unwrap().ollama.ollama_model.clone() };
                let request = GenerationRequest::new(model, context);
                Box::pin(async_stream::stream! {
                    let ollama = Ollama::default();
                    let mut stream = ollama.generate_stream(request).await.unwrap();

                    while let Some(Ok(res)) = stream.next().await {
                        for out in res {
                            yield out.response;
                        }
                    }
                })
            }
            TextGeneratorType::ShellScriptGenerator => {
                let custom_command = {
                    let settings = SETTINGS.lock().unwrap();
                    settings.custom_commands.custom_commands[
                        settings.custom_commands.index
                    ].command.clone()
                };
                // make child from custom_command
                let child = Command::new(&custom_command[0])
                    .args(&custom_command[1..])
                    .arg(&context)
                    .stdout(Stdio::piped())
                    .spawn()
                    .expect("Failed to spawn child process");

                let stdout = BufReader::new(child.stdout.expect("Failed to take stdout of child"));

                let stream = async_stream::stream! {
                    let mut reader = stdout;
                    let mut buffer = Vec::new();

                    loop {
                        buffer.clear();
                        let mut temp_buf = [0; 1024]; // Temporary buffer for each read
                        match reader.read(&mut temp_buf).await {
                            Ok(0) => break, // EOF reached
                            Ok(size) => {
                                buffer.extend_from_slice(&temp_buf[..size]);
                                yield String::from_utf8_lossy(&buffer).to_string();
                            },
                            Err(e) => {
                                eprintln!("Error reading from stdout: {}", e);
                                break;
                            }
                        }
                    }
                };

                Box::pin(stream)
            }
        }
    }
}
