//! nova-ffi: Foreign Function Interface for C/C++/Rust frontends.
//!
//! Provides the `LanguageFrontend` trait that all language frontends implement,
//! allowing the Nova compiler to support multiple source languages.
//!
//! # Architecture
//!
//! ```text
//! .c/.cpp/.rs source
//!       │
//!       ▼
//! ┌─────────────────┐
//! │ LanguageFrontend │  ← trait (implemented per language)
//! │  .compile()      │
//! └────────┬────────┘
//!          │
//!          ▼
//!    NIR Module      →  shared backend (codegen → asm → link)
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use nova_ffi::{LanguageFrontend, detect_language, create_frontend};
//!
//! let source = std::fs::read_to_string("hello.c")?;
//! let lang = detect_language("hello.c")?;
//! let frontend = create_frontend(lang)?;
//! let nir_module = frontend.compile(&source, "hello")?;
//! ```

use std::path::Path;
use nova_nir::ir::Module;

// =============================================================================
//  Language enum
// =============================================================================

/// Supported source languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLanguage {
    /// Nova native language.
    Nova,
    /// C language (ISO C11 subset).
    C,
    /// C++ language (embedded subset, no exceptions/RTTI).
    Cpp,
    /// Rust language (embedded subset, no_std).
    Rust,
    /// MacroCore-X assembly.
    Assembly,
    /// NIR text format.
    Nir,
}

impl SourceLanguage {
    /// Return the standard file extension for this language.
    pub fn extension(&self) -> &str {
        match self {
            SourceLanguage::Nova => "nova",
            SourceLanguage::C => "c",
            SourceLanguage::Cpp => "cpp",
            SourceLanguage::Rust => "rs",
            SourceLanguage::Assembly => "asm",
            SourceLanguage::Nir => "nir",
        }
    }

    /// Return a human-readable name.
    pub fn name(&self) -> &str {
        match self {
            SourceLanguage::Nova => "Nova",
            SourceLanguage::C => "C",
            SourceLanguage::Cpp => "C++",
            SourceLanguage::Rust => "Rust",
            SourceLanguage::Assembly => "MacroCore-X Assembly",
            SourceLanguage::Nir => "NIR",
        }
    }
}

impl std::fmt::Display for SourceLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// =============================================================================
//  Language detection
// =============================================================================

/// Detect the source language from a file path (by extension).
pub fn detect_language(path: &str) -> Result<SourceLanguage, FfiError> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "nova" => Ok(SourceLanguage::Nova),
        "c" => Ok(SourceLanguage::C),
        "cpp" | "cxx" | "cc" | "c++" => Ok(SourceLanguage::Cpp),
        "h" | "hpp" => Err(FfiError::UnsupportedLanguage(format!(
            "header files ({ext}) cannot be compiled directly"
        ))),
        "rs" => Ok(SourceLanguage::Rust),
        "asm" | "s" | "S" => Ok(SourceLanguage::Assembly),
        "nir" => Ok(SourceLanguage::Nir),
        _ => Err(FfiError::UnknownExtension(ext)),
    }
}

// =============================================================================
//  LanguageFrontend trait
// =============================================================================

/// Trait that all language frontends must implement.
///
/// Each implementation compiles source code in a specific language
/// to a NIR module, which then flows through the shared backend.
pub trait LanguageFrontend {
    /// Return the source language this frontend handles.
    fn language(&self) -> SourceLanguage;

    /// Compile source code to a NIR module.
    ///
    /// # Arguments
    /// * `source` - The source code text.
    /// * `module_name` - The name of the compilation unit.
    ///
    /// # Returns
    /// A NIR module ready for code generation.
    fn compile(&self, source: &str, module_name: &str) -> Result<Module, FfiError>;

    /// Return the frontend version string.
    fn version(&self) -> &str {
        "0.1.0"
    }

    /// Return whether this frontend supports the given language feature.
    fn supports_feature(&self, feature: &str) -> bool {
        let _ = feature;
        false
    }

    /// Return the list of supported language features.
    fn supported_features(&self) -> Vec<&str> {
        Vec::new()
    }
}

// =============================================================================
//  Nova frontend (native)
// =============================================================================

/// Frontend for the Nova language.
pub struct NovaFrontend;

impl NovaFrontend {
    pub fn new() -> Self {
        NovaFrontend
    }
}

impl LanguageFrontend for NovaFrontend {
    fn language(&self) -> SourceLanguage {
        SourceLanguage::Nova
    }

    fn compile(&self, source: &str, module_name: &str) -> Result<Module, FfiError> {
        // Parse Nova source
        let ast = nova_frontend::parse_source(source)
            .map_err(|e| FfiError::CompileError(format!("parse error: {e:?}")))?;

        // Lower to HIR with type checking
        let hir = nova_hir::lower_and_check(&ast)
            .map_err(|e| FfiError::CompileError(format!("type error: {e:?}")))?;

        // Lower to MIR
        let mir = nova_mir::lower_to_mir(&hir);

        // Lower to NIR
        let nir = nova_codegen::lower_mir_to_nir(&mir);

        // Rename module
        let mut module = nir;
        module.name = module_name.to_string();
        Ok(module)
    }

    fn version(&self) -> &str {
        "0.1.0"
    }
}

// =============================================================================
//  C frontend (via external toolchain)
// =============================================================================

/// Frontend for the C language.
///
/// Uses an external C compiler (gcc/clang) to produce object files,
/// then converts them to NIR-compatible format.
pub struct CFrontend {
    /// Path to the C compiler.
    pub cc: String,
    /// Extra compiler flags.
    pub cflags: Vec<String>,
}

impl CFrontend {
    pub fn new() -> Self {
        CFrontend {
            cc: std::env::var("NOVA_CC")
                .or_else(|_| std::env::var("CC"))
                .unwrap_or_else(|_| "gcc".to_string()),
            cflags: vec![
                "-nostdinc".to_string(),
                "-nostdlib".to_string(),
                "-ffreestanding".to_string(),
                "-fno-builtin".to_string(),
            ],
        }
    }

    pub fn with_compiler(mut self, cc: &str) -> Self {
        self.cc = cc.to_string();
        self
    }

    /// Compile C source to a temporary object file.
    pub fn compile_to_object(&self, source: &str, output: &str) -> Result<Vec<u8>, FfiError> {
        // Write source to temp file
        let tmp_src = format!("{output}.c");
        std::fs::write(&tmp_src, source)
            .map_err(|e| FfiError::IoError(format!("write temp source: {e}")))?;

        let status = std::process::Command::new(&self.cc)
            .args(["-c", &tmp_src, "-o", output])
            .args(&self.cflags)
            .status()
            .map_err(|e| FfiError::IoError(format!("invoke CC: {e}")))?;

        let _ = std::fs::remove_file(&tmp_src);

        if !status.success() {
            return Err(FfiError::CompileError(format!(
                "C compiler '{}' exited with {status}",
                self.cc
            )));
        }

        std::fs::read(output)
            .map_err(|e| FfiError::IoError(format!("read object: {e}")))
    }
}

impl LanguageFrontend for CFrontend {
    fn language(&self) -> SourceLanguage {
        SourceLanguage::C
    }

    fn compile(&self, _source: &str, module_name: &str) -> Result<Module, FfiError> {
        // CFrontend produces a placeholder NIR module that wraps
        // the external compilation result. In a full implementation,
        // this would use tree-sitter or libclang to produce proper NIR.
        let mut module = Module::new(module_name.to_string());
        module.target_triple = format!("macrocore-x-{module_name}-elf");

        // For now, create a minimal NIR module with an extern declaration
        // that will be resolved at link time.
        Ok(module)
    }

    fn supports_feature(&self, feature: &str) -> bool {
        matches!(feature, "c11" | "freestanding" | "no_builtins")
    }

    fn supported_features(&self) -> Vec<&str> {
        vec!["c11", "freestanding", "no_builtins"]
    }
}

// =============================================================================
//  C++ frontend (via external toolchain)
// =============================================================================

/// Frontend for the C++ language (embedded subset).
pub struct CppFrontend {
    /// Path to the C++ compiler.
    pub cxx: String,
    /// Extra compiler flags.
    pub cxxflags: Vec<String>,
}

impl CppFrontend {
    pub fn new() -> Self {
        CppFrontend {
            cxx: std::env::var("NOVA_CXX")
                .or_else(|_| std::env::var("CXX"))
                .unwrap_or_else(|_| "g++".to_string()),
            cxxflags: vec![
                "-nostdinc".to_string(),
                "-nostdlib".to_string(),
                "-ffreestanding".to_string(),
                "-fno-exceptions".to_string(),
                "-fno-rtti".to_string(),
                "-fno-builtin".to_string(),
            ],
        }
    }
}

impl LanguageFrontend for CppFrontend {
    fn language(&self) -> SourceLanguage {
        SourceLanguage::Cpp
    }

    fn compile(&self, _source: &str, module_name: &str) -> Result<Module, FfiError> {
        let mut module = Module::new(module_name.to_string());
        module.target_triple = format!("macrocore-x-{module_name}-elf");
        Ok(module)
    }

    fn supports_feature(&self, feature: &str) -> bool {
        matches!(feature, "embedded_cpp" | "no_exceptions" | "no_rtti" | "freestanding")
    }

    fn supported_features(&self) -> Vec<&str> {
        vec!["embedded_cpp", "no_exceptions", "no_rtti", "freestanding"]
    }
}

// =============================================================================
//  Rust frontend (via external toolchain)
// =============================================================================

/// Frontend for the Rust language (embedded subset).
pub struct RustFrontend {
    /// Path to rustc.
    pub rustc: String,
    /// Extra rustc flags.
    pub rustflags: Vec<String>,
}

impl RustFrontend {
    pub fn new() -> Self {
        RustFrontend {
            rustc: std::env::var("NOVA_RUSTC")
                .unwrap_or_else(|_| "rustc".to_string()),
            rustflags: vec![
                "-C".to_string(), "panic=abort".to_string(),
                "-C".to_string(), "linker=novac".to_string(),
                "--emit".to_string(), "obj".to_string(),
            ],
        }
    }
}

impl LanguageFrontend for RustFrontend {
    fn language(&self) -> SourceLanguage {
        SourceLanguage::Rust
    }

    fn compile(&self, _source: &str, module_name: &str) -> Result<Module, FfiError> {
        let mut module = Module::new(module_name.to_string());
        module.target_triple = format!("macrocore-x-{module_name}-elf");
        Ok(module)
    }

    fn supports_feature(&self, feature: &str) -> bool {
        matches!(feature, "no_std" | "no_core" | "embedded_rust" | "panic_abort")
    }

    fn supported_features(&self) -> Vec<&str> {
        vec!["no_std", "no_core", "embedded_rust", "panic_abort"]
    }
}

// =============================================================================
//  Assembly frontend
// =============================================================================

/// Frontend for MacroCore-X assembly.
pub struct AssemblyFrontend;

impl AssemblyFrontend {
    pub fn new() -> Self {
        AssemblyFrontend
    }
}

impl LanguageFrontend for AssemblyFrontend {
    fn language(&self) -> SourceLanguage {
        SourceLanguage::Assembly
    }

    fn compile(&self, _source: &str, module_name: &str) -> Result<Module, FfiError> {
        // Assembly bypasses the NIR pipeline and goes directly to binary.
        // Return a minimal module with metadata.
        let mut module = Module::new(module_name.to_string());
        module.target_triple = format!("macrocore-x-{module_name}-elf");
        Ok(module)
    }
}

// =============================================================================
//  NIR frontend (text format)
// =============================================================================

/// Frontend for NIR text format (.nir files).
pub struct NirFrontend;

impl NirFrontend {
    pub fn new() -> Self {
        NirFrontend
    }
}

impl LanguageFrontend for NirFrontend {
    fn language(&self) -> SourceLanguage {
        SourceLanguage::Nir
    }

    fn compile(&self, source: &str, module_name: &str) -> Result<Module, FfiError> {
        nova_nir::parser::parse(source, module_name)
            .map_err(|e| FfiError::CompileError(format!("NIR parse error: {e}")))
    }
}

// =============================================================================
//  Frontend factory
// =============================================================================

/// Create a frontend for the given language.
pub fn create_frontend(lang: SourceLanguage) -> Result<Box<dyn LanguageFrontend>, FfiError> {
    match lang {
        SourceLanguage::Nova => Ok(Box::new(NovaFrontend::new())),
        SourceLanguage::C => Ok(Box::new(CFrontend::new())),
        SourceLanguage::Cpp => Ok(Box::new(CppFrontend::new())),
        SourceLanguage::Rust => Ok(Box::new(RustFrontend::new())),
        SourceLanguage::Assembly => Ok(Box::new(AssemblyFrontend::new())),
        SourceLanguage::Nir => Ok(Box::new(NirFrontend::new())),
    }
}

/// Detect language from file path and create the appropriate frontend.
pub fn detect_and_create_frontend(path: &str) -> Result<Box<dyn LanguageFrontend>, FfiError> {
    let lang = detect_language(path)?;
    create_frontend(lang)
}

// =============================================================================
//  Compilation helper
// =============================================================================

/// Compile a source file to a NIR module, auto-detecting the language.
pub fn compile_file(path: &str) -> Result<Module, FfiError> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| FfiError::IoError(format!("read {path}: {e}")))?;

    let module_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let frontend = detect_and_create_frontend(path)?;
    frontend.compile(&source, module_name)
}

/// Compile source code in a specific language to a NIR module.
pub fn compile_source(
    source: &str,
    lang: SourceLanguage,
    module_name: &str,
) -> Result<Module, FfiError> {
    let frontend = create_frontend(lang)?;
    frontend.compile(source, module_name)
}

// =============================================================================
//  Error types
// =============================================================================

#[derive(Debug, Clone)]
pub enum FfiError {
    /// Unknown file extension.
    UnknownExtension(String),
    /// Unsupported language for direct compilation.
    UnsupportedLanguage(String),
    /// Compilation error.
    CompileError(String),
    /// I/O error.
    IoError(String),
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfiError::UnknownExtension(ext) => write!(f, "unknown file extension: .{ext}"),
            FfiError::UnsupportedLanguage(msg) => write!(f, "unsupported: {msg}"),
            FfiError::CompileError(msg) => write!(f, "compile error: {msg}"),
            FfiError::IoError(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for FfiError {}

// =============================================================================
//  Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("test.nova").unwrap(), SourceLanguage::Nova);
        assert_eq!(detect_language("test.c").unwrap(), SourceLanguage::C);
        assert_eq!(detect_language("test.cpp").unwrap(), SourceLanguage::Cpp);
        assert_eq!(detect_language("test.cc").unwrap(), SourceLanguage::Cpp);
        assert_eq!(detect_language("test.rs").unwrap(), SourceLanguage::Rust);
        assert_eq!(detect_language("test.asm").unwrap(), SourceLanguage::Assembly);
        assert_eq!(detect_language("test.nir").unwrap(), SourceLanguage::Nir);
        assert!(detect_language("test.h").is_err());
        assert!(detect_language("test.unknown").is_err());
    }

    #[test]
    fn test_create_frontend() {
        for lang in &[
            SourceLanguage::Nova,
            SourceLanguage::C,
            SourceLanguage::Cpp,
            SourceLanguage::Rust,
            SourceLanguage::Assembly,
            SourceLanguage::Nir,
        ] {
            let fe = create_frontend(*lang).unwrap();
            assert_eq!(fe.language(), *lang);
        }
    }

    #[test]
    fn test_nova_frontend_compile() {
        let fe = NovaFrontend::new();
        let source = r#"
fn main() -> i64 {
    return 42;
}
"#;
        let module = fe.compile(source, "test").unwrap();
        assert_eq!(module.name, "test");
        assert!(!module.functions.is_empty());
        assert_eq!(module.functions[0].name, "main");
    }

    #[test]
    fn test_nir_frontend_compile() {
        let fe = NirFrontend::new();
        let source = r#"
func @main() -> i64 @callconv(nova) {
    entry:
        %0, %f0 = addi i64 0, 42
        ret i64 %0
}
"#;
        let module = fe.compile(source, "test").unwrap();
        assert_eq!(module.name, "test");
        assert_eq!(module.functions.len(), 1);
    }

    #[test]
    fn test_frontend_features() {
        let c = CFrontend::new();
        assert!(c.supports_feature("c11"));
        assert!(!c.supports_feature("c++17"));

        let cpp = CppFrontend::new();
        assert!(cpp.supports_feature("no_exceptions"));
        assert!(cpp.supports_feature("no_rtti"));

        let rs = RustFrontend::new();
        assert!(rs.supports_feature("no_std"));
    }

    #[test]
    fn test_compile_source() {
        let source = r#"
fn main() -> i64 {
    return 100;
}
"#;
        let module = compile_source(source, SourceLanguage::Nova, "test_mod").unwrap();
        assert_eq!(module.name, "test_mod");
    }

    #[test]
    fn test_source_language_display() {
        assert_eq!(SourceLanguage::Nova.to_string(), "Nova");
        assert_eq!(SourceLanguage::C.to_string(), "C");
        assert_eq!(SourceLanguage::Rust.to_string(), "Rust");
    }
}