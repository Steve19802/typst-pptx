use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use typst_frontend::dump::{self, DumpOptions};
use typst_frontend::ir::TextModel;
use typst_frontend::{DiagnosticFormat, ExportWorld, PagedDocument, TouyingCheck, WorldOptions};

/// The typst version must match the `typst` pin in the workspace Cargo.toml;
/// tino compares it with `typst --version`.
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (typst 0.15.1)");

/// Separator of path lists in flags and environment variables, as in typst.
const PATH_SEP: char = if cfg!(windows) { ';' } else { ':' };

#[derive(Parser)]
#[command(
    name = "typst-pptx",
    version = VERSION,
    about = "Export Typst/Touying decks to editable PowerPoint files"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a deck to .pptx.
    Compile {
        #[command(flatten)]
        world: WorldArgs,
        /// Output file; defaults to the input with a .pptx extension.
        output: Option<PathBuf>,
        /// Also write the intermediate representation as JSON.
        #[arg(long, value_name = "FILE.json")]
        emit_ir: Option<PathBuf>,
        /// How Touying subslides are exported (auto = slides until click
        /// animations are implemented).
        #[arg(long, value_enum, default_value_t = Animations::Auto)]
        animations: Animations,
        /// Place text for this application's text layout (libreoffice: for
        /// LibreOffice Impress, used by the CI visual diff).
        #[arg(long, value_enum, default_value_t = TextModelArg::Powerpoint)]
        text_model: TextModelArg,
        /// Embed the fonts of the text boxes where licence and format allow.
        #[arg(long, value_enum, default_value_t = EmbedFonts::Auto)]
        embed_fonts: EmbedFonts,
    },
    /// Print the laid-out frames of a deck (debugging aid, unstable format).
    Dump {
        #[command(flatten)]
        world: WorldArgs,
        /// Pages to print, 1-based (default: all).
        #[arg(long, value_delimiter = ',')]
        pages: Vec<usize>,
        /// Print tag start/end lines.
        #[arg(long)]
        tags: bool,
        /// Print the open elements next to every item.
        #[arg(long)]
        paths: bool,
    },
}

/// Same flags and environment variables as `typst compile`.
#[derive(Args)]
struct WorldArgs {
    /// Main .typ file.
    input: PathBuf,
    /// Project root (default: the input's directory).
    #[arg(long, env = "TYPST_ROOT")]
    root: Option<PathBuf>,
    /// sys.inputs entry, KEY=VALUE (repeatable).
    #[arg(long = "input", value_name = "KEY=VALUE", value_parser = parse_input)]
    inputs: Vec<(String, String)>,
    /// Extra font directories.
    #[arg(long = "font-path", env = "TYPST_FONT_PATHS", value_delimiter = PATH_SEP)]
    font_paths: Vec<PathBuf>,
    /// Don't use system fonts.
    #[arg(long, env = "TYPST_IGNORE_SYSTEM_FONTS")]
    ignore_system_fonts: bool,
    /// Don't use Typst's embedded fonts.
    #[arg(long, env = "TYPST_IGNORE_EMBEDDED_FONTS")]
    ignore_embedded_fonts: bool,
    /// Local package directory.
    #[arg(long, env = "TYPST_PACKAGE_PATH")]
    package_path: Option<PathBuf>,
    /// Downloaded package cache directory.
    #[arg(long, env = "TYPST_PACKAGE_CACHE_PATH")]
    package_cache_path: Option<PathBuf>,
    /// Fixed creation time as a UNIX timestamp.
    #[arg(long, env = "SOURCE_DATE_EPOCH")]
    creation_timestamp: Option<i64>,
    #[arg(long, value_enum, default_value_t = Format::Human)]
    diagnostic_format: Format,
}

#[derive(Clone, Copy, ValueEnum)]
enum Animations {
    Auto,
    None,
    Slides,
}

#[derive(Clone, Copy, ValueEnum)]
enum EmbedFonts {
    Auto,
    None,
}

#[derive(Clone, Copy, ValueEnum)]
enum TextModelArg {
    Powerpoint,
    Libreoffice,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Human,
    Short,
}

fn parse_input(raw: &str) -> Result<(String, String), String> {
    match raw.split_once('=') {
        Some((key, value)) if !key.trim().is_empty() => {
            Ok((key.trim().to_string(), value.to_string()))
        }
        _ => Err("expected KEY=VALUE".into()),
    }
}

const TOUYING_UPGRADE_HINT: &str = "update the import to @preview/touying:0.8.0; breaking changes are listed at https://touying-typ.github.io/docs/changelog";

/// Exit codes (plan §8): 1 = compile error, 2 = export error.
const COMPILE_ERROR: u8 = 1;
const EXPORT_ERROR: u8 = 2;

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Compile {
            world,
            output,
            emit_ir,
            animations,
            text_model,
            embed_fonts,
        } => compile(&world).and_then(|doc| {
            let output = output.unwrap_or_else(|| world.input.with_extension("pptx"));
            let options = typst_pptx::Options {
                animations: match animations {
                    Animations::Auto => animate::Mode::Auto,
                    Animations::None => animate::Mode::None,
                    Animations::Slides => animate::Mode::Slides,
                },
                text_model: match text_model {
                    TextModelArg::Powerpoint => TextModel::PowerPoint,
                    TextModelArg::Libreoffice => TextModel::LibreOffice,
                },
                embed_fonts: matches!(embed_fonts, EmbedFonts::Auto),
            };
            export(&doc, &output, emit_ir.as_deref(), options).map_err(|err| {
                eprintln!("error: {err}");
                EXPORT_ERROR
            })
        }),
        Command::Dump {
            world,
            pages,
            tags,
            paths,
        } => compile(&world).map(|doc| {
            print!(
                "{}",
                dump::dump(
                    &doc,
                    &pages,
                    DumpOptions {
                        tag_paths: paths,
                        tags
                    }
                )
            );
        }),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn export(
    doc: &PagedDocument,
    output: &Path,
    emit_ir: Option<&Path>,
    options: typst_pptx::Options,
) -> Result<(), String> {
    let export = typst_pptx::export(doc, options)?;
    for (kind, count) in &export.report.skipped {
        eprintln!("warning: {count} {kind} item(s) not exported yet");
    }
    for warning in &export.report.warnings {
        eprintln!("warning: {warning}");
    }
    if let Some(path) = emit_ir {
        let json = serde_json::to_string_pretty(&export.deck).map_err(|err| err.to_string())?;
        fs::write(path, json + "\n")
            .map_err(|err| format!("cannot write {} ({err})", path.display()))?;
    }
    let file = File::create(output)
        .map_err(|err| format!("cannot create {} ({err})", output.display()))?;
    ooxml::write_pptx(&export.deck, &export.properties, BufWriter::new(file))
        .map_err(|err| format!("cannot write {} ({err})", output.display()))
}

/// Compiles the deck, printing diagnostics; `Err` holds the exit code.
fn compile(args: &WorldArgs) -> Result<PagedDocument, u8> {
    let options = WorldOptions {
        input: args.input.clone(),
        root: args.root.clone(),
        inputs: args.inputs.clone(),
        font_paths: args.font_paths.clone(),
        ignore_system_fonts: args.ignore_system_fonts,
        ignore_embedded_fonts: args.ignore_embedded_fonts,
        package_path: args.package_path.clone(),
        package_cache_path: args.package_cache_path.clone(),
        creation_timestamp: args.creation_timestamp,
    };
    let world = ExportWorld::new(&options).map_err(|err| {
        eprintln!("error: {err}");
        COMPILE_ERROR
    })?;
    let format = match args.diagnostic_format {
        Format::Human => DiagnosticFormat::Human,
        Format::Short => DiagnosticFormat::Short,
    };
    let warned = typst_frontend::compile(&world);
    let result = match warned.output {
        Ok(doc) => {
            typst_frontend::emit_diagnostics(&world, &[], &warned.warnings, format);
            Ok(doc)
        }
        Err(errors) => {
            typst_frontend::emit_diagnostics(&world, &errors, &warned.warnings, format);
            Err(COMPILE_ERROR)
        }
    };
    // Checked after compiling so the message also follows the compile errors
    // an old Touying typically causes under Typst 0.15.
    match typst_frontend::check_touying(&world) {
        TouyingCheck::Ok => result,
        TouyingCheck::Untested(message) => {
            eprintln!("warning: {message}");
            result
        }
        TouyingCheck::Unsupported(message) => {
            eprintln!("error: {message}");
            eprintln!("  = hint: {TOUYING_UPGRADE_HINT}");
            Err(result.err().unwrap_or(EXPORT_ERROR))
        }
    }
}
