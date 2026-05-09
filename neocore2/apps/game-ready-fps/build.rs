use std::{
    env,
    fs,
    path::{Path, PathBuf},
};

fn main() {
    if env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest_dir.join("Cargo.toml");
    println!("cargo:rerun-if-changed={}", cargo_toml.display());

    let pkg_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "app".to_string());
    let pkg_version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    let pkg_description = env::var("CARGO_PKG_DESCRIPTION").unwrap_or_default();
    let pkg_authors = env::var("CARGO_PKG_AUTHORS").unwrap_or_default();
    let pkg_repository = env::var("CARGO_PKG_REPOSITORY").unwrap_or_default();
    let pkg_homepage = env::var("CARGO_PKG_HOMEPAGE").unwrap_or_default();
    let pkg_license = env::var("CARGO_PKG_LICENSE").unwrap_or_default();

    let meta = read_winres_meta(&cargo_toml);

    let icon_rel = meta
        .icon
        .unwrap_or_else(|| panic_missing_icon(&cargo_toml));

    let icon_path = manifest_dir.join(&icon_rel);
    if !icon_path.exists() {
        panic!(
            "Windows icon not found: {} (from Cargo.toml: package.metadata.newengine.winres.icon = {:?})",
            icon_path.display(),
            icon_rel
        );
    }
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let (v1, v2, v3, v4) = parse_version4(&pkg_version);
    let ver4_str = format!("{v1}.{v2}.{v3}.{v4}");

    let company = meta
        .company_name
        .as_deref()
        .or_else(|| first_author(&pkg_authors))
        .unwrap_or("");

    let file_desc = meta
        .file_description
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let d = pkg_description.trim();
            if d.is_empty() { None } else { Some(d) }
        })
        .unwrap_or(pkg_name.as_str());

    let product_name = meta.product_name.as_deref().unwrap_or(pkg_name.as_str());
    let internal_name = meta.internal_name.as_deref().unwrap_or(pkg_name.as_str());
    let original_filename = meta
        .original_filename
        .clone()
        .unwrap_or_else(|| format!("{pkg_name}.exe"));

    let sfi_block = meta
        .string_file_info_block
        .as_deref()
        .unwrap_or("041904B0");

    let lang_id = meta.lang_id.unwrap_or(0x0419);
    let codepage = meta.codepage.unwrap_or(1200);

    let icon_rc_path = path_for_rc(&icon_path);

    let rc = format!(
        r#"
1 ICON "{icon}"

VS_VERSION_INFO VERSIONINFO
 FILEVERSION {v1},{v2},{v3},{v4}
 PRODUCTVERSION {v1},{v2},{v3},{v4}
 FILEFLAGSMASK 0x3fL
 FILEFLAGS 0x0L
 FILEOS 0x40004L
 FILETYPE 0x1L
 FILESUBTYPE 0x0L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "{sfi_block}"
    BEGIN
      VALUE "CompanyName", "{company}\0"
      VALUE "FileDescription", "{file_desc}\0"
      VALUE "FileVersion", "{file_version}\0"
      VALUE "InternalName", "{internal_name}\0"
      VALUE "OriginalFilename", "{original_filename}\0"
      VALUE "ProductName", "{product_name}\0"
      VALUE "ProductVersion", "{product_version}\0"
      VALUE "LegalCopyright", "{copyright}\0"
      VALUE "License", "{license}\0"
      VALUE "Repository", "{repository}\0"
      VALUE "Homepage", "{homepage}\0"
    END
  END

  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", {lang_id}, {codepage}
  END
END
"#,
        icon = escape_rc(&icon_rc_path),
        sfi_block = escape_rc(sfi_block),
        company = escape_rc(company),
        file_desc = escape_rc(file_desc),
        file_version = escape_rc(&ver4_str),
        internal_name = escape_rc(internal_name),
        original_filename = escape_rc(&original_filename),
        product_name = escape_rc(product_name),
        product_version = escape_rc(&ver4_str),
        copyright = escape_rc(&default_copyright(company)),
        license = escape_rc(pkg_license.trim()),
        repository = escape_rc(pkg_repository.trim()),
        homepage = escape_rc(pkg_homepage.trim()),
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let rc_path = out_dir.join("winres.rc");
    fs::write(&rc_path, rc).expect("write winres.rc");

    embed_resource::compile(rc_path.to_str().expect("rc_path utf-8"), embed_resource::NONE);
}

#[derive(Default)]
struct WinResMeta {
    icon: Option<String>,
    product_name: Option<String>,
    file_description: Option<String>,
    company_name: Option<String>,
    original_filename: Option<String>,
    internal_name: Option<String>,
    string_file_info_block: Option<String>,
    lang_id: Option<u32>,
    codepage: Option<u32>,
}

fn panic_missing_icon(cargo_toml: &Path) -> ! {
    panic!(
        "Windows icon path must be set in Cargo.toml:\n\
         [package.metadata.newengine.winres]\n\
         icon = \"assets/editor.ico\"\n\
         file: {}",
        cargo_toml.display()
    );
}

fn read_winres_meta(cargo_toml: &Path) -> WinResMeta {
    let text = fs::read_to_string(cargo_toml).expect("read Cargo.toml");
    let doc: toml::Value = text.parse().expect("parse Cargo.toml as TOML");

    let mut m = WinResMeta::default();

    let base = doc
        .get("package")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("newengine"))
        .and_then(|v| v.get("winres"));

    let Some(base) = base else { return m; };

    m.icon = base.get("icon").and_then(as_str).map(str::to_owned);
    m.product_name = base.get("product_name").and_then(as_str).map(str::to_owned);
    m.file_description = base.get("file_description").and_then(as_str).map(str::to_owned);
    m.company_name = base.get("company_name").and_then(as_str).map(str::to_owned);
    m.original_filename = base.get("original_filename").and_then(as_str).map(str::to_owned);
    m.internal_name = base.get("internal_name").and_then(as_str).map(str::to_owned);
    m.string_file_info_block = base
        .get("string_file_info_block")
        .and_then(as_str)
        .map(str::to_owned);

    m.lang_id = base.get("lang_id").and_then(as_u32);
    m.codepage = base.get("codepage").and_then(as_u32);

    m
}

fn as_str(v: &toml::Value) -> Option<&str> {
    v.as_str()
}

fn as_u32(v: &toml::Value) -> Option<u32> {
    if let Some(i) = v.as_integer() {
        if i >= 0 && i <= i64::from(u32::MAX) {
            return Some(i as u32);
        }
    }
    None
}

fn parse_version4(v: &str) -> (u16, u16, u16, u16) {
    let mut nums = [0u16; 4];
    let mut idx = 0usize;

    for part in v.split('.') {
        if idx >= 4 {
            break;
        }
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            break;
        }
        nums[idx] = digits.parse::<u16>().unwrap_or(0);
        idx += 1;
    }

    (nums[0], nums[1], nums[2], nums[3])
}

fn first_author(authors: &str) -> Option<&str> {
    let s = authors.trim();
    if s.is_empty() {
        return None;
    }
    s.split(';')
        .next()
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
}

fn default_copyright(company: &str) -> String {
    if company.trim().is_empty() {
        "Copyright (C)".to_string()
    } else {
        format!("Copyright (C) {company}")
    }
}

fn path_for_rc(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn escape_rc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\r' => {}
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}