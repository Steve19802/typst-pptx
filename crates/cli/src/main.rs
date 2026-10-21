use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use typst_frontend::dump::{self, DumpOptions};
use typst_frontend::ir::{TableMode, TextModel};
use typst_frontend::{
    DiagnosticFormat, ExportWorld, PagedDocument, SourceDiagnostic, Span, TouyingCheck,
    WorldOptions,
};

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
        /// exact = keep Typst's line breaks; flow = paragraphs wrap in
        /// PowerPoint (for decks that will be heavily edited).
        #[arg(long, value_enum, default_value_t = TextMode::Exact)]
        text: TextMode,
        /// Embed the fonts of the text boxes where licence and format allow.
        #[arg(long, value_enum, default_value_t = EmbedFonts::Auto)]
        embed_fonts: EmbedFonts,
        /// SVG images: auto = simple ones (up to --svg-max-nodes) as native
        /// shapes, picture = all as pictures, native = every convertible one.
        #[arg(long, value_enum, default_value_t = SvgMode::Auto)]
        svg: SvgMode,
        /// Largest SVG (in nodes) converted to native shapes in auto mode.
        #[arg(long, default_value_t = 60)]
        svg_max_nodes: usize,
        /// Tables: auto = native PowerPoint tables where they keep Typst's
        /// look, native = also where PowerPoint might grow a row, shapes =
        /// rectangles, lines and text boxes.
        #[arg(long, value_enum, default_value_t = TablesArg::Auto)]
        tables: TablesArg,
        /// Write what was exported how (per slide, fallbacks, fonts) as JSON.
        #[arg(long, value_name = "FILE.json")]
        report: Option<PathBuf>,
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
enum TextMode {
    Exact,
    Flow,
}

#[derive(Clone, Copy, ValueEnum)]
enum TablesArg {
    Auto,
    Native,
    Shapes,
}

#[derive(Clone, Copy, ValueEnum)]
enum SvgMode {
    Auto,
    Picture,
    Native,
}

#[derive(Clone, Copy, ValueEnum)]
enum EmbedFonts {
    Auto,
    Never,
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
            text,
            embed_fonts,
            svg,
            svg_max_nodes,
            tables,
            report,
        } => compile(&world).and_then(|(export_world, doc)| {
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
                flow: matches!(text, TextMode::Flow),
                svg_max_nodes: match svg {
                    SvgMode::Auto => svg_max_nodes,
                    SvgMode::Picture => 0,
                    SvgMode::Native => usize::MAX,
                },
                tables: match tables {
                    TablesArg::Auto => TableMode::Auto,
                    TablesArg::Native => TableMode::Native,
                    TablesArg::Shapes => TableMode::Shapes,
                },
            };
            let paths = Outputs {
                pptx: &output,
                ir: emit_ir.as_deref(),
                report: report.as_deref(),
            };
            export(&export_world, &doc, paths, options, format(&world)).map_err(|err| {
                eprintln!("error: {err}");
                EXPORT_ERROR
            })
        }),
        Command::Dump {
            world,
            pages,
            tags,
            paths,
        } => compile(&world).map(|(_, doc)| {
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

struct Outputs<'a> {
    pptx: &'a Path,
    ir: Option<&'a Path>,
    report: Option<&'a Path>,
}

fn export(
    world: &ExportWorld,
    doc: &PagedDocument,
    paths: Outputs,
    options: typst_pptx::Options,
    format: DiagnosticFormat,
) -> Result<(), String> {
    let export = typst_pptx::export(doc, options)?;
    typst_frontend::emit_diagnostics(world, &[], &warnings(&export), format);
    let write_json = |path: &Path, json: String| {
        fs::write(path, json + "\n")
            .map_err(|err| format!("cannot write {} ({err})", path.display()))
    };
    if let Some(path) = paths.ir {
        write_json(
            path,
            serde_json::to_string_pretty(&export.deck).map_err(|e| e.to_string())?,
        )?;
    }
    if let Some(path) = paths.report {
        let report = typst_pptx::report::build(&export, world);
        write_json(
            path,
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
        )?;
    }
    let output = paths.pptx;
    let file = File::create(output)
        .map_err(|err| format!("cannot create {} ({err})", output.display()))?;
    ooxml::write_pptx(&export.deck, &export.properties, BufWriter::new(file))
        .map_err(|err| format!("cannot write {} ({err})", output.display()))
}

/// Export problems as Typst warnings at the element's source (plan §8).
/// Baked equations are by design and not reported.
fn warnings(export: &typst_pptx::Export) -> Vec<SourceDiagnostic> {
    let report = &export.report;
    let exported = |page: usize| export.deck.slides.iter().any(|slide| slide.page == page);
    let mut out: Vec<SourceDiagnostic> = Vec::new();
    for baked in &report.baked {
        if baked.reason != "equation" && exported(baked.at.page) {
            out.push(
                SourceDiagnostic::warning(
                    baked.at.span,
                    format!("exported as a picture: {}", baked.reason),
                )
                .with_hint("it looks the same but isn't editable in PowerPoint"),
            );
        }
    }
    for at in &report.line_mode {
        if exported(at.page) {
            out.push(
                SourceDiagnostic::warning(at.span, "paragraph exported line by line")
                    .with_hint("PowerPoint would draw one of its lines at another width"),
            );
        }
    }
    for at in &report.list_fallbacks {
        if exported(at.page) {
            out.push(
                SourceDiagnostic::warning(at.span, "list exported without native bullets")
                    .with_hint("its markers are text boxes"),
            );
        }
    }
    for fallback in &report.table_fallbacks {
        if exported(fallback.at.page) {
            out.push(
                SourceDiagnostic::warning(
                    fallback.at.span,
                    "table exported as shapes and text boxes",
                )
                .with_hint(format!("not a PowerPoint table: {}", fallback.reason)),
            );
        }
    }
    for (font, reason) in &report.fonts_not_embedded {
        out.push(SourceDiagnostic::warning(
            Span::detached(),
            format!("font \"{font}\" is not embedded: {reason}"),
        ));
    }
    out.dedup_by(|a, b| a.span == b.span && a.message == b.message);
    out
}

fn format(args: &WorldArgs) -> DiagnosticFormat {
    match args.diagnostic_format {
        Format::Human => DiagnosticFormat::Human,
        Format::Short => DiagnosticFormat::Short,
    }
}

/// Compiles the deck, printing diagnostics; `Err` holds the exit code.
fn compile(args: &WorldArgs) -> Result<(ExportWorld, PagedDocument), u8> {
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
    let format = format(args);
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
    let result = match typst_frontend::check_touying(&world) {
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
    };
    result.map(|doc| (world, doc))
}
