use clap::{Parser, Subcommand};
use moredata_audio::{play_once, probe, render_wav};
use moredata_core::{CompileOptions, CompiledGraph, Diagnostics, Project, StatusReport};
use moredata_plugin::builtin_catalog;
use moredata_runtime::Runtime;
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "moredata", version, about = "MoreData control plane")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Status,
    System,
    Audio {
        #[command(subcommand)]
        cmd: AudioCmd,
    },
    Graph {
        #[command(subcommand)]
        cmd: GraphCmd,
    },
    Diagnostics,
    Logs,
    Patch {
        file: PathBuf,
    },
    Render {
        file: PathBuf,
        #[arg(long, short)]
        output: PathBuf,
        #[arg(long, default_value_t = 1.0)]
        seconds: f32,
    },
    Play {
        file: PathBuf,
        #[arg(long, default_value_t = 1.0)]
        seconds: f32,
    },
    /// Stream a project to the device until interrupted (lock-free RT link).
    Serve {
        file: PathBuf,
    },
    Plugins,
}

#[derive(Subcommand)]
enum AudioCmd {
    Status,
}

#[derive(Subcommand)]
enum GraphCmd {
    Validate { file: PathBuf },
    Show { file: PathBuf },
}

#[derive(Serialize)]
struct JsonErr {
    ok: bool,
    error: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string(&JsonErr {
                        ok: false,
                        error: e.clone()
                    })
                    .unwrap_or_else(|_| format!("{{\"ok\":false,\"error\":{e:?}}}"))
                );
            } else {
                eprintln!("error: {e}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), String> {
    match &cli.cmd {
        Commands::Status => print_json(cli.json, &StatusReport::current()),
        Commands::System => {
            #[derive(Serialize)]
            struct Sys {
                rustc: &'static str,
                engine: &'static str,
                pd_coupled: bool,
                host: String,
            }
            print_json(
                cli.json,
                &Sys {
                    rustc: "1.98",
                    engine: moredata_core::ENGINE,
                    pd_coupled: false,
                    host: std::env::consts::OS.into(),
                },
            )
        }
        Commands::Audio {
            cmd: AudioCmd::Status,
        } => print_json(cli.json, &probe()),
        Commands::Graph { cmd } => match cmd {
            GraphCmd::Validate { file } => {
                let g = load_graph(file)?;
                g.validate().map_err(|e| e.to_string())?;
                #[derive(Serialize)]
                struct Ok {
                    ok: bool,
                    nodes: usize,
                    connections: usize,
                }
                print_json(
                    cli.json,
                    &Ok {
                        ok: true,
                        nodes: g.nodes().len(),
                        connections: g.connections().len(),
                    },
                )
            }
            GraphCmd::Show { file } => {
                let g = load_graph(file)?;
                print_json(cli.json, &g.to_project())
            }
        },
        Commands::Diagnostics => print_json(cli.json, &Diagnostics::default()),
        Commands::Logs => {
            #[derive(Serialize)]
            struct Logs {
                backend: &'static str,
                note: &'static str,
            }
            print_json(
                cli.json,
                &Logs {
                    backend: "stderr",
                    note: "realtime path has no logs; control plane uses this CLI",
                },
            )
        }
        Commands::Patch { file } => {
            let g = load_graph(file)?;
            print_json(cli.json, &g.to_project())
        }
        Commands::Render {
            file,
            output,
            seconds,
        } => {
            let g = load_graph(file)?;
            let sr = g.sample_rate;
            let (cg, st) =
                CompiledGraph::compile(&g, CompileOptions::default()).map_err(|e| e.to_string())?;
            let mut rt = Runtime::new(cg, st, "wav");
            let frames = render_wav(&mut rt, output, *seconds, sr).map_err(|e| e.to_string())?;
            #[derive(Serialize)]
            struct Out {
                ok: bool,
                output: String,
                frames: u64,
                sample_rate: u32,
            }
            print_json(
                cli.json,
                &Out {
                    ok: true,
                    output: output.display().to_string(),
                    frames,
                    sample_rate: sr,
                },
            )
        }
        Commands::Play { file, seconds } => {
            let g = load_graph(file)?;
            let (cg, st) =
                CompiledGraph::compile(&g, CompileOptions::default()).map_err(|e| e.to_string())?;
            let rt = Runtime::new(cg, st, "cpal");
            play_once(rt, *seconds).map_err(|e| e.to_string())?;
            #[derive(Serialize)]
            struct Out {
                ok: bool,
                backend: &'static str,
                seconds: f32,
            }
            print_json(
                cli.json,
                &Out {
                    ok: true,
                    backend: "cpal",
                    seconds: *seconds,
                },
            )
        }
        Commands::Serve { file } => {
            let g = load_graph(file)?;
            let (cg, st) =
                CompiledGraph::compile(&g, CompileOptions::default()).map_err(|e| e.to_string())?;
            let rt = Runtime::new(cg, st, "cpal");
            let session = moredata_audio::play(rt).map_err(|e| e.to_string())?;
            #[derive(Serialize)]
            struct Out {
                ok: bool,
                backend: &'static str,
                note: &'static str,
            }
            print_json(
                cli.json,
                &Out {
                    ok: true,
                    backend: "cpal",
                    note: "streaming; ctrl-c to stop",
                },
            )?;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let _ = session.poll_retired();
            }
        }
        Commands::Plugins => print_json(cli.json, &builtin_catalog()),
    }
}

fn load_graph(path: &PathBuf) -> Result<moredata_core::Graph, String> {
    let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let p = Project::from_json(&s).map_err(|e| e.to_string())?;
    p.to_graph().map_err(|e| e.to_string())
}

fn print_json<T: Serialize>(force: bool, v: &T) -> Result<(), String> {
    let _ = force;
    println!(
        "{}",
        serde_json::to_string_pretty(v).map_err(|e| e.to_string())?
    );
    Ok(())
}
