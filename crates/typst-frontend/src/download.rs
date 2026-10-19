//! Package downloads over rustls. typst-kit's `SystemDownloader` needs
//! OpenSSL, which the static musl release builds can't link (plan §2.1).

use std::any::Any;
use std::io::{self, Read};

use typst::syntax::package::PackageSpec;
use typst_kit::downloader::Downloader;

pub struct RustlsDownloader {
    agent: ureq::Agent,
}

impl RustlsDownloader {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .user_agent(concat!("typst-pptx/", env!("CARGO_PKG_VERSION")))
            .try_proxy_from_env(true)
            .build();
        Self { agent }
    }
}

impl Downloader for RustlsDownloader {
    fn stream(&self, key: &dyn Any, url: &str) -> io::Result<(Option<usize>, Box<dyn Read>)> {
        // Same first line as the typst CLI's progress output.
        if let Some(spec) = key.downcast_ref::<PackageSpec>() {
            eprintln!("downloading {spec}");
        }
        let response = self.agent.get(url).call().map_err(|err| match err {
            ureq::Error::Status(404, _) => io::Error::new(io::ErrorKind::NotFound, err),
            err => io::Error::other(err),
        })?;
        let len = response
            .header("Content-Length")
            .and_then(|value| value.parse().ok());
        Ok((len, response.into_reader()))
    }
}
