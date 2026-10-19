//! The [`World`] the exporter compiles in. Fonts, packages and inputs resolve
//! the same way as in the typst CLI, so a deck that compiles with
//! `typst compile` compiles here with the same flags.

use std::path::{Path, PathBuf};

use typst::diag::FileResult;
use typst::foundations::{Bytes, Datetime, Dict, Duration, IntoValue, Str};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::diagnostics::DiagnosticWorld;
use typst_kit::files::{FileStore, FsRoot, SystemFiles};
use typst_kit::fonts::{self, FontStore};
use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

use crate::download::RustlsDownloader;

/// `sys.inputs` the exporter adds unless the caller sets them (plan §4, §6.1).
const DEFAULT_INPUTS: [(&str, &str); 2] = [("export", "pptx"), ("export-mode", "presentation")];

/// How to set up an [`ExportWorld`]; mirrors the typst CLI's world flags.
#[derive(Debug, Clone, Default)]
pub struct WorldOptions {
    /// The main `.typ` file.
    pub input: PathBuf,
    /// Project root; defaults to the input's directory.
    pub root: Option<PathBuf>,
    /// `sys.inputs` key/value pairs, in command-line order.
    pub inputs: Vec<(String, String)>,
    /// Extra font directories, searched recursively, in order.
    pub font_paths: Vec<PathBuf>,
    pub ignore_system_fonts: bool,
    pub ignore_embedded_fonts: bool,
    /// Local package directory (`--package-path`).
    pub package_path: Option<PathBuf>,
    /// Downloaded package cache (`--package-cache-path`).
    pub package_cache_path: Option<PathBuf>,
    /// Fixed "now" as a UNIX timestamp, for reproducible output.
    pub creation_timestamp: Option<i64>,
}

pub struct ExportWorld {
    root: PathBuf,
    workdir: Option<PathBuf>,
    main: FileId,
    library: LazyHash<Library>,
    fonts: FontStore,
    files: FileStore<SystemFiles>,
    time: Time,
}

impl ExportWorld {
    pub fn new(options: &WorldOptions) -> Result<Self, String> {
        let input = options
            .input
            .canonicalize()
            .map_err(|err| format!("input file not found ({err}): {}", options.input.display()))?;
        let root = match &options.root {
            Some(root) => root
                .canonicalize()
                .map_err(|err| format!("root not found ({err}): {}", root.display()))?,
            None => input.parent().map(Path::to_path_buf).unwrap_or_default(),
        };
        let vpath = VirtualPath::virtualize(&root, &input)
            .map_err(|_| "input file must be contained in project root".to_string())?;
        let main = RootedPath::new(VirtualRoot::Project, vpath).intern();

        let mut inputs = Dict::new();
        for (key, value) in DEFAULT_INPUTS {
            inputs.insert(Str::from(key), Str::from(value).into_value());
        }
        for (key, value) in &options.inputs {
            inputs.insert(
                Str::from(key.as_str()),
                Str::from(value.as_str()).into_value(),
            );
        }
        let library = Library::builder().with_inputs(inputs).build();

        let mut fonts = FontStore::new();
        for path in &options.font_paths {
            fonts.extend(fonts::scan(path));
        }
        if !options.ignore_system_fonts {
            fonts.extend(fonts::system());
        }
        if !options.ignore_embedded_fonts {
            fonts.extend(fonts::embedded());
        }

        let packages = SystemPackages::from_parts(
            options
                .package_path
                .clone()
                .map(FsPackages::new)
                .or_else(FsPackages::system_data),
            options
                .package_cache_path
                .clone()
                .map(FsPackages::new)
                .or_else(FsPackages::system_cache),
            UniversePackages::new(RustlsDownloader::new()),
        );
        let files = FileStore::new(SystemFiles::new(FsRoot::new(root.clone()), packages));

        let time = match options.creation_timestamp {
            Some(ts) => Time::fixed_timestamp(ts).map_err(|err| err.to_string())?,
            None => Time::system(),
        };

        Ok(Self {
            root,
            workdir: std::env::current_dir().ok(),
            main,
            library: LazyHash::new(library),
            fonts,
            files,
            time,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The real path of a project or package file.
    pub fn path(&self, id: FileId) -> FileResult<PathBuf> {
        self.files.loader().resolve(id)
    }
}

impl World for ExportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.time.today(offset)
    }
}

impl DiagnosticWorld for ExportWorld {
    /// Same naming as the typst CLI: package files as `@ns/name:ver/path`,
    /// project files relative to the working directory.
    fn name(&self, id: FileId) -> String {
        if let VirtualRoot::Package(spec) = id.root() {
            return format!("{spec}{}", id.vpath().get_with_slash());
        }
        let Ok(path) = self.path(id) else {
            return id.vpath().get_without_slash().to_string();
        };
        self.workdir
            .as_deref()
            .and_then(|dir| path.strip_prefix(dir).ok())
            .unwrap_or(&path)
            .display()
            .to_string()
    }
}
