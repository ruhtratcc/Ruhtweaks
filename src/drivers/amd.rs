//! The published AMD bootstrapper chooses compatible Recommended packages.
use crate::{
    Result, fail, security,
    state::{Store, atomic_json},
};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
pub const PAGE: &str = "https://www.amd.com/en/support/download/drivers.html";
const PUBLISHERS: &[&str] = &["Advanced Micro Devices"];
pub fn allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && matches!(url.host_str(), Some("www.amd.com" | "drivers.amd.com"))
}
pub fn resolve(html: &str) -> Result<String> {
    let mut links = html
        .split(['"', '\'', '<', '>', ' ', '\n', '\r'])
        .filter_map(|part| {
            let url = reqwest::Url::parse(part).ok()?;
            let file = url.path().rsplit('/').next()?;
            (allowed(&url)
                && url.host_str() == Some("drivers.amd.com")
                && url.path().starts_with("/drivers/installer/")
                && file.starts_with("amd-software-adrenalin-edition-")
                && file.contains("-minimalsetup-")
                && file.ends_with("_web.exe")
                && url.query().is_none()
                && url.fragment().is_none())
            .then(|| url.to_string())
        })
        .collect::<Vec<_>>();
    links.sort();
    links.dedup();
    if links.len() != 1 {
        return Err(fail(
            "A página AMD mudou ou não oferece um único Auto-Detect reconhecido. Use o seletor oficial; nenhum endereço foi adivinhado.",
        ));
    }
    Ok(links.remove(0))
}
fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .https_only(true)
        .user_agent(concat!("Ruhtweaks/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !allowed(attempt.url()) {
                attempt.error("Redirecionamento AMD não autorizado")
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}
pub fn prepare(store: &Store, cancel: &AtomicBool, log: &mut dyn FnMut(String)) -> Result<PathBuf> {
    let client = client()?;
    let mut html = String::new();
    client
        .get(PAGE)
        .send()?
        .error_for_status()?
        .take(4 * 1024 * 1024)
        .read_to_string(&mut html)?;
    let source = resolve(&html)?;
    let filename = reqwest::Url::parse(&source)
        .map_err(|_| fail("URL AMD inválida"))?
        .path()
        .rsplit('/')
        .next()
        .ok_or_else(|| fail("Nome AMD ausente"))?
        .to_owned();
    if !filename
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err(fail("Nome de arquivo AMD inválido"));
    }
    let folder = store.root.join("downloads/amd");
    std::fs::create_dir_all(&folder)?;
    let dest = folder.join(filename);
    if dest.is_file() {
        security::preserve_download_protection(&dest, &source)?;
        security::verify_publisher(&dest, PUBLISHERS)?;
        return Ok(dest);
    }
    let mut response = client
        .get(&source)
        .header(reqwest::header::REFERER, PAGE)
        .send()?
        .error_for_status()?;
    let total = response.content_length();
    const LIMIT: u64 = 256 * 1024 * 1024;
    if total.is_some_and(|n| n > LIMIT) {
        return Err(fail("Ferramenta AMD excede o limite de 256 MiB"));
    }
    let mut temp = tempfile::NamedTempFile::new_in(&folder)?;
    let mut done = 0u64;
    let mut buffer = [0; 65536];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(fail("Download AMD cancelado"));
        }
        let n = response.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done > LIMIT {
            return Err(fail("Download AMD excede limite"));
        }
        temp.write_all(&buffer[..n])?;
        log(format!("AMD Auto-Detect: {done} bytes / {total:?}"));
    }
    temp.as_file().sync_all()?;
    let publisher = security::verify_publisher(temp.path(), PUBLISHERS)?;
    let digest = security::digest_file(temp.path())?;
    temp.persist(&dest).map_err(|e| e.error)?;
    security::preserve_download_protection(&dest, &source)?;
    security::verify_publisher(&dest, PUBLISHERS)?;
    if security::digest_file(&dest)? != digest {
        return Err(fail("Ferramenta AMD mudou durante a inspeção de segurança"));
    }
    atomic_json(
        &dest.with_extension("source.json"),
        &serde_json::json!({"schema_version":1,"source":source,"sha256":digest,"publisher":publisher,"page":PAGE,"published_hash":false}),
    )?;
    Ok(dest)
}
