//! SymbolicCompressor: Token-efficient code representation
//!
//! Compresses full source code into symbolic representations (signatures only)
//! to achieve 90%+ token reduction while preserving structural understanding.
//!
//! ## Design Philosophy (Rich Hickey)
//! - **Data-Oriented**: Treats code as data to be transformed
//! - **Lazy Compilation**: Regex patterns compiled once via `lazy_static`
//! - **Composable**: Each language compressor is independent
//! - **Preserves Intent**: Keeps signatures, types, and relationships

use regex::Regex;
use std::sync::LazyLock;
use crate::memory::GraphNode;

// ============================================================================
// Lazy-compiled Regex Patterns (compiled once, reused forever)
// ============================================================================

mod patterns {
    use super::*;

    // Rust patterns
    pub static RUST_STRUCT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:///[^\n]*\n)*(?:pub(?:\([^)]*\))?\s+)?struct\s+(\w+)(?:<[^>]+>)?\s*(?:\{([^}]*)\}|\([^)]*\)|;)").unwrap()
    });
    pub static RUST_ENUM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:///[^\n]*\n)*(?:pub(?:\([^)]*\))?\s+)?enum\s+(\w+)(?:<[^>]+>)?\s*\{([^}]*)\}").unwrap()
    });
    pub static RUST_TRAIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub(?:\([^)]*\))?\s+)?trait\s+(\w+)(?:<[^>]+>)?(?:\s*:\s*[^{]+)?\s*\{").unwrap()
    });
    pub static RUST_IMPL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^impl(?:<[^>]+>)?\s+(\w+)(?:<[^>]+>)?(?:\s+for\s+(\w+)(?:<[^>]+>)?)?\s*\{").unwrap()
    });
    pub static RUST_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)\s*(?:<[^>]+>)?\s*\(([^)]*)\)(?:\s*->\s*([^{;]+))?").unwrap()
    });
    pub static RUST_CONST: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub(?:\([^)]*\))?\s+)?const\s+(\w+)\s*:\s*([^=]+)").unwrap()
    });
    pub static RUST_TYPE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub(?:\([^)]*\))?\s+)?type\s+(\w+)(?:<[^>]+>)?\s*=\s*([^;]+)").unwrap()
    });
    pub static RUST_USE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^use\s+([^;]+)").unwrap()
    });
    pub static RUST_MOD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)").unwrap()
    });
    pub static RUST_FIELD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:\s*([^,}\n]+)").unwrap()
    });

    // TypeScript patterns
    pub static TS_INTERFACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+)?interface\s+(\w+)(?:<[^>]+>)?(?:\s+extends\s+([^{]+))?\s*\{").unwrap()
    });
    pub static TS_TYPE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+)?type\s+(\w+)(?:<[^>]+>)?\s*=\s*([^;]+)").unwrap()
    });
    pub static TS_CLASS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+)?(?:abstract\s+)?class\s+(\w+)(?:<[^>]+>)?(?:\s+extends\s+(\w+))?(?:\s+implements\s+([^{]+))?\s*\{").unwrap()
    });
    pub static TS_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+)?(?:async\s+)?function\s+(\w+)(?:<[^>]+>)?\s*\(([^)]*)\)(?:\s*:\s*([^{]+))?").unwrap()
    });
    pub static TS_ARROW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+)?(?:const|let)\s+(\w+)(?:\s*:\s*([^=]+))?\s*=\s*(?:async\s+)?\([^)]*\)\s*(?::\s*[^=]+)?\s*=>").unwrap()
    });
    pub static TS_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^import\s+(?:\{[^}]+\}|[^{]+)\s+from\s+[']([^']+)").unwrap()
    });

    // JavaScript patterns (reuse some TS patterns)
    pub static JS_REQUIRE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)require\s*\(\s*[']([^']+)[']\s*\)").unwrap()
    });

    // Python patterns
    pub static PY_CLASS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^class\s+(\w+)(?:\(([^)]*)\))?:").unwrap()
    });
    pub static PY_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:    |\t)?(?:async\s+)?def\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*([^:]+))?:").unwrap()
    });
    pub static PY_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:from\s+(\S+)\s+)?import\s+([^\n]+)").unwrap()
    });

    // Go patterns
    pub static GO_STRUCT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^type\s+(\w+)\s+struct\s*\{([^}]*)\}").unwrap()
    });
    pub static GO_INTERFACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^type\s+(\w+)\s+interface\s*\{([^}]*)\}").unwrap()
    });
    pub static GO_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^func\s+(?:\((\w+)\s+\*?(\w+)\)\s+)?(\w+)\s*\(([^)]*)\)(?:\s*\(([^)]+)\)|\s+([^\s{]+))?").unwrap()
    });
    pub static GO_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^\s*"([^"]+)""#).unwrap()
    });

    // Markdown patterns
    pub static MD_HEADING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(#{1,6})\s+(.+)$").unwrap()
    });
    pub static MD_CODE_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^```(\w+)?").unwrap()
    });

    // Elixir patterns
    pub static EX_MODULE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^defmodule\s+([A-Z][\w.]+)\s+do").unwrap()
    });
    pub static EX_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:def|defp)\s+(\w+)\s*\(([^)]*)\)").unwrap()
    });
    pub static EX_MACRO: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*defmacro\s+(\w+)\s*\(([^)]*)\)").unwrap()
    });
    pub static EX_STRUCT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*defstruct\s+\[([^\]]+)\]").unwrap()
    });
    pub static EX_USE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:use|import|alias|require)\s+([A-Z][\w.]+)").unwrap()
    });
    pub static EX_CALLBACK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*@callback\s+(\w+)\s*\(([^)]*)\)").unwrap()
    });

    // Gleam patterns
    pub static GLEAM_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^import\s+([\w/]+)").unwrap()
    });
    pub static GLEAM_TYPE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub\s+)?type\s+(\w+)(?:\([^)]*\))?\s*\{").unwrap()
    });
    pub static GLEAM_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub\s+)?fn\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*(\w+))?").unwrap()
    });
    pub static GLEAM_CONST: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub\s+)?const\s+(\w+)(?::\s*(\w+))?\s*=").unwrap()
    });

    // Clojure patterns
    pub static CLJ_NS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\(ns\s+([\w.-]+)").unwrap()
    });
    pub static CLJ_DEFN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\(defn-?\s+(\S+)\s*(?:\[([^\]]*)\])?").unwrap()
    });
    pub static CLJ_DEF: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\(def\s+(\S+)").unwrap()
    });
    pub static CLJ_DEFMACRO: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\(defmacro\s+(\S+)\s*\[([^\]]*)\]").unwrap()
    });
    pub static CLJ_DEFPROTOCOL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\(defprotocol\s+(\S+)").unwrap()
    });
    pub static CLJ_DEFRECORD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\(defrecord\s+(\S+)\s*\[([^\]]*)\]").unwrap()
    });
    pub static CLJ_REQUIRE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m):require\s+\[([^\]]+)\]").unwrap()
    });

    // Java patterns
    pub static JAVA_PACKAGE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^package\s+([\w.]+);").unwrap()
    });
    pub static JAVA_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^import\s+(?:static\s+)?([\w.]+);").unwrap()
    });
    pub static JAVA_CLASS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:public\s+)?(?:abstract\s+)?(?:final\s+)?class\s+(\w+)(?:<[^>]+>)?(?:\s+extends\s+(\w+))?(?:\s+implements\s+([^{]+))?\s*\{").unwrap()
    });
    pub static JAVA_INTERFACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:public\s+)?interface\s+(\w+)(?:<[^>]+>)?(?:\s+extends\s+([^{]+))?\s*\{").unwrap()
    });
    pub static JAVA_ENUM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:public\s+)?enum\s+(\w+)(?:\s+implements\s+([^{]+))?\s*\{").unwrap()
    });
    pub static JAVA_METHOD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:public|private|protected)?\s*(?:static\s+)?(?:final\s+)?(?:synchronized\s+)?(?:<[^>]+>\s+)?(\w+(?:<[^>]+>)?)\s+(\w+)\s*\(([^)]*)\)").unwrap()
    });
    pub static JAVA_RECORD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:public\s+)?record\s+(\w+)\s*\(([^)]*)\)").unwrap()
    });

    // Bash/Shell patterns
    pub static BASH_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:function\s+)?(\w+)\s*\(\s*\)\s*\{").unwrap()
    });
    pub static BASH_ALIAS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^alias\s+(\w+)=").unwrap()
    });
    pub static BASH_EXPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^export\s+(\w+)=").unwrap()
    });
    pub static BASH_SOURCE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^(?:source|\\.)\s+['""]?([^'"";\s]+)"#).unwrap()
    });

    // Zig patterns
    pub static ZIG_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub\s+)?fn\s+(\w+)\s*\(([^)]*)\)(?:\s*([^{]+))?\s*\{").unwrap()
    });
    pub static ZIG_STRUCT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub\s+)?const\s+(\w+)\s*=\s*(?:packed\s+)?struct\s*\{").unwrap()
    });
    pub static ZIG_ENUM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:pub\s+)?const\s+(\w+)\s*=\s*enum(?:\([^)]+\))?\s*\{").unwrap()
    });
    pub static ZIG_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"@import\s*\(\s*"([^"]+)""#).unwrap()
    });

    // C/C++ patterns
    pub static C_INCLUDE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^#include\s+[<"]([^>"]+)[>"]"#).unwrap()
    });
    pub static C_DEFINE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^#define\s+(\w+)").unwrap()
    });
    pub static C_STRUCT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:typedef\s+)?struct\s+(\w+)").unwrap()
    });
    pub static C_ENUM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:typedef\s+)?enum\s+(\w+)").unwrap()
    });
    pub static C_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:static\s+)?(?:inline\s+)?(?:const\s+)?(\w+(?:\s*\*)*)\s+(\w+)\s*\(([^)]*)\)\s*\{").unwrap()
    });
    pub static CPP_CLASS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:template\s*<[^>]+>\s*)?class\s+(\w+)(?:\s*:\s*(?:public|private|protected)\s+(\w+))?").unwrap()
    });
    pub static CPP_NAMESPACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^namespace\s+(\w+)").unwrap()
    });

    // SQL patterns
    pub static SQL_CREATE_TABLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?[`'\x22]?(\w+)[`'\x22]?").unwrap()
    });
    pub static SQL_CREATE_INDEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)CREATE\s+(?:UNIQUE\s+)?INDEX\s+(?:IF\s+NOT\s+EXISTS\s+)?[`'\x22]?(\w+)[`'\x22]?").unwrap()
    });
    pub static SQL_CREATE_VIEW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)CREATE\s+(?:OR\s+REPLACE\s+)?VIEW\s+[`'\x22]?(\w+)[`'\x22]?").unwrap()
    });
    pub static SQL_CREATE_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)CREATE\s+(?:OR\s+REPLACE\s+)?FUNCTION\s+[`'\x22]?(\w+)[`'\x22]?").unwrap()
    });

    // CSS patterns
    pub static CSS_SELECTOR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^([.#]?[\w-]+(?:\s*,\s*[.#]?[\w-]+)*)\s*\{").unwrap()
    });
    pub static CSS_VARIABLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*--([a-zA-Z][\w-]*)\s*:").unwrap()
    });
    pub static CSS_MEDIA: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^@media\s*([^{]+)").unwrap()
    });
    pub static CSS_KEYFRAMES: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^@keyframes\s+(\w+)").unwrap()
    });

    // Dockerfile patterns
    pub static DOCKER_FROM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^FROM\s+(\S+)").unwrap()
    });
    pub static DOCKER_EXPOSE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^EXPOSE\s+(\d+)").unwrap()
    });
    pub static DOCKER_ENV: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^ENV\s+(\w+)").unwrap()
    });
    pub static DOCKER_CMD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^(?:CMD|ENTRYPOINT)\s+(.+)$").unwrap()
    });

    // Swift patterns
    pub static SWIFT_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^import\s+(\w+)").unwrap()
    });
    pub static SWIFT_CLASS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:public\s+|private\s+|internal\s+|open\s+)?(?:final\s+)?class\s+(\w+)(?:\s*:\s*([^{]+))?").unwrap()
    });
    pub static SWIFT_STRUCT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:public\s+|private\s+)?struct\s+(\w+)(?:\s*:\s*([^{]+))?").unwrap()
    });
    pub static SWIFT_ENUM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:public\s+|private\s+)?enum\s+(\w+)(?:\s*:\s*([^{]+))?").unwrap()
    });
    pub static SWIFT_PROTOCOL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:public\s+)?protocol\s+(\w+)(?:\s*:\s*([^{]+))?").unwrap()
    });
    pub static SWIFT_FUNC: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:public\s+|private\s+|internal\s+|@\w+\s+)*func\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*([^{]+))?").unwrap()
    });
    pub static SWIFT_EXTENSION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^extension\s+(\w+)(?:\s*:\s*([^{]+))?").unwrap()
    });

    // Kotlin patterns
    pub static KOTLIN_PACKAGE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^package\s+([\w.]+)").unwrap()
    });
    pub static KOTLIN_CLASS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:data\s+|sealed\s+|open\s+|abstract\s+)?class\s+(\w+)(?:\s*:\s*([^{(]+))?").unwrap()
    });
    pub static KOTLIN_OBJECT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:companion\s+)?object\s+(\w+)?").unwrap()
    });
    pub static KOTLIN_FUN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:suspend\s+)?(?:fun\s+)(\w+)\s*\(([^)]*)\)(?:\s*:\s*([^\n{=]+))?").unwrap()
    });
    pub static KOTLIN_INTERFACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^interface\s+(\w+)(?:\s*:\s*([^{]+))?").unwrap()
    });

    // Ruby patterns
    pub static RUBY_CLASS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^class\s+(\w+)(?:\s*<\s*(\w+))?").unwrap()
    });
    pub static RUBY_MODULE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^module\s+(\w+)").unwrap()
    });
    pub static RUBY_DEF: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*def\s+(\w+[?!]?)(?:\(([^)]*)\))?").unwrap()
    });
    pub static RUBY_REQUIRE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^require(?:_relative)?\s+[']([^']+)").unwrap()
    });

    // PHP patterns
    pub static PHP_NAMESPACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^namespace\s+([\w\\]+);").unwrap()
    });
    pub static PHP_CLASS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:abstract\s+|final\s+)?class\s+(\w+)(?:\s+extends\s+(\w+))?(?:\s+implements\s+([^{]+))?").unwrap()
    });
    pub static PHP_INTERFACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^interface\s+(\w+)(?:\s+extends\s+([^{]+))?").unwrap()
    });
    pub static PHP_TRAIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^trait\s+(\w+)").unwrap()
    });
    pub static PHP_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:public|private|protected|static|\s)*function\s+(\w+)\s*\(([^)]*)\)(?:\s*:\s*\??(\w+))?").unwrap()
    });

    // Lua patterns
    pub static LUA_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:local\s+)?function\s+(\w+(?:\.\w+)*)\s*\(([^)]*)\)").unwrap()
    });
    pub static LUA_LOCAL_FN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^local\s+(\w+)\s*=\s*function\s*\(([^)]*)\)").unwrap()
    });
    pub static LUA_REQUIRE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"require\s*\(\s*[']([^']+)").unwrap()
    });
    pub static LUA_TABLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:local\s+)?(\w+)\s*=\s*\{").unwrap()
    });

    // Vue/Svelte/Astro SFC patterns
    pub static SFC_SCRIPT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?ms)<script[^>]*>(.+?)</script>").unwrap()
    });
    pub static SFC_STYLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"<style[^>]*(?:lang=["'](\w+)["'])?[^>]*>"#).unwrap()
    });
    pub static SFC_COMPONENT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^<([A-Z]\w+)").unwrap()
    });
    pub static VUE_DEFINE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)define(Props|Emits|Expose|Slots)\s*(?:<([^>]+)>)?\s*\(").unwrap()
    });

    // GraphQL patterns
    pub static GQL_TYPE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^type\s+(\w+)(?:\s+implements\s+([^{]+))?\s*\{").unwrap()
    });
    pub static GQL_INTERFACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^interface\s+(\w+)\s*\{").unwrap()
    });
    pub static GQL_INPUT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^input\s+(\w+)\s*\{").unwrap()
    });
    pub static GQL_ENUM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^enum\s+(\w+)\s*\{").unwrap()
    });

    // Terraform/HCL patterns
    pub static TF_RESOURCE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^resource\s+"(\w+)"\s+"(\w+)""#).unwrap()
    });
    pub static TF_DATA: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^data\s+"(\w+)"\s+"(\w+)""#).unwrap()
    });
    pub static TF_VARIABLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^variable\s+"(\w+)""#).unwrap()
    });
    pub static TF_OUTPUT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^output\s+"(\w+)""#).unwrap()
    });
    pub static TF_MODULE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^module\s+"(\w+)""#).unwrap()
    });
}


/// Symbolic representation of code - signatures without implementations
pub struct SymbolicCompressor;

/// Compression output with metadata
#[derive(Debug, Clone)]
pub struct CompressedOutput {
    pub symbols: String,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub line_count: usize,
    pub symbol_count: usize,
}

impl Default for CompressedOutput {
    fn default() -> Self {
        Self {
            symbols: String::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            line_count: 0,
            symbol_count: 0,
        }
    }
}

impl CompressedOutput {
    // to_string method removed as it was unused
}

impl SymbolicCompressor {
    /// Compress source code to symbolic representation
    /// Returns a compact string with only signatures, types, and relationships
    pub fn compress(content: &str, lang: &str) -> String {
        match lang {
            // Systems languages
            "rs" => Self::compress_rust(content),
            "go" => Self::compress_go(content),
            "zig" => Self::compress_zig(content),
            "c" | "h" => Self::compress_c(content),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Self::compress_cpp(content),
            
            // JVM languages
            "java" => Self::compress_java(content),
            "kt" | "kts" => Self::compress_kotlin(content),
            "scala" | "sc" => Self::compress_scala(content),
            
            // Web languages
            "ts" | "tsx" => Self::compress_typescript(content),
            "js" | "jsx" | "mjs" | "cjs" => Self::compress_javascript(content),
            "vue" => Self::compress_vue(content),
            "svelte" => Self::compress_svelte(content),
            "astro" => Self::compress_astro(content),
            
            // Scripting languages
            "py" | "pyi" => Self::compress_python(content),
            "rb" | "rake" | "gemspec" => Self::compress_ruby(content),
            "php" => Self::compress_php(content),
            "lua" => Self::compress_lua(content),
            "sh" | "bash" | "zsh" => Self::compress_bash(content),
            
            // Functional languages
            "ex" | "exs" => Self::compress_elixir(content),
            "gleam" => Self::compress_gleam(content),
            "clj" | "cljs" | "cljc" | "edn" => Self::compress_clojure(content),
            
            // Apple ecosystem
            "swift" => Self::compress_swift(content),
            
            // Data/Config languages
            "sql" => Self::compress_sql(content),
            "graphql" | "gql" => Self::compress_graphql(content),
            "tf" | "hcl" => Self::compress_terraform(content),
            "json" => Self::compress_json(content),
            "yaml" | "yml" => Self::compress_yaml(content),
            "toml" => Self::compress_toml(content),
            
            // Styling
            "css" | "scss" | "sass" | "less" => Self::compress_css(content),
            
            // Documentation
            "md" | "mdx" => Self::compress_markdown(content),
            
            // Container
            "dockerfile" => Self::compress_dockerfile(content),
            
            _ => Self::compress_generic(content),
        }
    }



    /// Compress with detailed metadata output
    pub fn compress_detailed(content: &str, lang: &str) -> CompressedOutput {
        let mut output = CompressedOutput::default();
        output.line_count = content.lines().count();
        
        let compressed = Self::compress(content, lang);
        output.symbol_count = compressed.lines().count();
        output.symbols = compressed;
        
        output
    }

    /// Compress a vector of GraphNodes to a symbolic context string
    pub fn compress_nodes(nodes: &[GraphNode]) -> String {
        let mut output = String::new();
        let mut current_path = String::new();

        for node in nodes {
            // Group by file path
            if node.path != current_path {
                if !current_path.is_empty() {
                    output.push('\n');
                }
                output.push_str(&format!("## {}\n", node.path));
                current_path = node.path.clone();
            }

            // Format based on node type with proper indentation
            let prefix = match node.node_type.as_str() {
                "struct" | "class" | "enum" | "trait" | "interface" => "├─ ",
                "fn" | "function" | "def" | "method" => "│  ├─ ",
                "impl" => "├─ ",
                _ => "│  ",
            };

            match node.node_type.as_str() {
                "struct" => output.push_str(&format!("{}struct {}\n", prefix, node.signature)),
                "enum" => output.push_str(&format!("{}enum {}\n", prefix, node.signature)),
                "trait" => output.push_str(&format!("{}trait {}\n", prefix, node.signature)),
                "fn" | "function" | "method" => output.push_str(&format!("{}fn {}\n", prefix, node.signature)),
                "impl" => output.push_str(&format!("{}impl {}\n", prefix, node.signature)),
                "class" => output.push_str(&format!("{}class {}\n", prefix, node.signature)),
                "interface" => output.push_str(&format!("{}interface {}\n", prefix, node.signature)),
                "import" | "use" => output.push_str(&format!("{}use {}\n", prefix, node.signature)),
                "const" => output.push_str(&format!("{}const {}\n", prefix, node.signature)),
                "type" => output.push_str(&format!("{}type {}\n", prefix, node.signature)),
                _ => {}
            }

            // Add edges if present (dependencies)
            if !node.edges.is_empty() {
                output.push_str(&format!("│     └─ deps: {}\n", node.edges.join(", ")));
            }
        }

        output
    }

    /// Rust-specific compression with enhanced extraction
    fn compress_rust(content: &str) -> String {
        let mut output = String::new();
        let mut modules = Vec::new();
        let mut imports = Vec::new();

        // Extract modules
        for cap in patterns::RUST_MOD.captures_iter(content) {
            modules.push(cap[1].to_string());
        }
        if !modules.is_empty() {
            output.push_str(&format!("mods: {}\n", modules.join(", ")));
        }

        // Extract imports (collapsed to crate level)
        for cap in patterns::RUST_USE.captures_iter(content) {
            let path = cap[1].trim();
            if let Some(crate_name) = path.split("::").next() {
                let crate_name = crate_name.trim_start_matches('{');
                if !imports.contains(&crate_name.to_string()) && !crate_name.is_empty() {
                    imports.push(crate_name.to_string());
                }
            }
        }
        if !imports.is_empty() {
            output.push_str(&format!("uses: {}\n\n", imports.join(", ")));
        }

        // Extract structs with field names and types
        for cap in patterns::RUST_STRUCT.captures_iter(content) {
            let name = &cap[1];
            if let Some(body) = cap.get(2) {
                let fields = Self::extract_rust_struct_fields(body.as_str());
                if fields.is_empty() {
                    output.push_str(&format!("struct {}\n", name));
                } else {
                    output.push_str(&format!("struct {} {{ {} }}\n", name, fields.join(", ")));
                }
            } else {
                output.push_str(&format!("struct {}\n", name));
            }
        }

        // Extract enums with variant names
        for cap in patterns::RUST_ENUM.captures_iter(content) {
            let name = &cap[1];
            if let Some(body) = cap.get(2) {
                let variants = Self::extract_rust_enum_variants(body.as_str());
                if variants.is_empty() {
                    output.push_str(&format!("enum {}\n", name));
                } else {
                    output.push_str(&format!("enum {} {{ {} }}\n", name, variants.join(" | ")));
                }
            } else {
                output.push_str(&format!("enum {}\n", name));
            }
        }

        // Extract traits
        for cap in patterns::RUST_TRAIT.captures_iter(content) {
            output.push_str(&format!("trait {}\n", &cap[1]));
        }

        // Extract impl blocks
        for cap in patterns::RUST_IMPL.captures_iter(content) {
            if let Some(for_type) = cap.get(2) {
                output.push_str(&format!("impl {} for {}\n", &cap[1], for_type.as_str()));
            } else {
                output.push_str(&format!("impl {}\n", &cap[1]));
            }
        }

        // Extract function signatures
        for cap in patterns::RUST_FN.captures_iter(content) {
            let name = &cap[1];
            let params = Self::compress_params(&cap[2]);
            let ret = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("()");
            output.push_str(&format!("  fn {}({}) -> {}\n", name, params, ret));
        }

        // Extract constants
        for cap in patterns::RUST_CONST.captures_iter(content) {
            output.push_str(&format!("const {}: {}\n", &cap[1], cap[2].trim()));
        }

        // Extract type aliases
        for cap in patterns::RUST_TYPE.captures_iter(content) {
            output.push_str(&format!("type {} = {}\n", &cap[1], cap[2].trim()));
        }

        output
    }

    /// Extract field names with simplified types from Rust struct
    fn extract_rust_struct_fields(struct_body: &str) -> Vec<String> {
        let mut fields = Vec::new();
        
        for cap in patterns::RUST_FIELD.captures_iter(struct_body) {
            let field = &cap[1];
            let ftype = cap.get(2).map(|m| Self::simplify_type(m.as_str())).unwrap_or_default();
            
            if field != "pub" && !field.is_empty() {
                if ftype.is_empty() {
                    fields.push(field.to_string());
                } else {
                    fields.push(format!("{}: {}", field, ftype));
                }
            }
        }
        
        fields
    }

    /// Extract enum variant names
    fn extract_rust_enum_variants(enum_body: &str) -> Vec<String> {
        let mut variants = Vec::new();
        
        for line in enum_body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            
            // Extract variant name (before any { or ( or ,)
            let name = trimmed
                .split(|c| c == '{' || c == '(' || c == ',')
                .next()
                .unwrap_or("")
                .trim();
            
            if !name.is_empty() && name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                variants.push(name.to_string());
            }
        }
        
        variants
    }

    /// Simplify a type annotation (remove lifetimes, shorten paths)
    fn simplify_type(t: &str) -> String {
        let t = t.trim();
        
        // Remove lifetimes
        let re_lifetime = Regex::new(r"'\w+\s*").unwrap();
        let t = re_lifetime.replace_all(t, "");
        
        // Shorten common types
        let t = t
            .replace("String", "Str")
            .replace("Vec<", "[")
            .replace("Option<", "?")
            .replace("Result<", "Res<")
            .replace("HashMap<", "Map<")
            .replace("HashSet<", "Set<");
        
        // Remove full paths (keep last segment)
        if t.contains("::") {
            t.split("::").last().unwrap_or(&t).to_string()
        } else {
            t.trim().to_string()
        }
    }

    /// Compress function parameters (keep names and simplified types)
    fn compress_params(params: &str) -> String {
        if params.trim().is_empty() {
            return String::new();
        }

        let parts: Vec<&str> = params.split(',').collect();
        let mut compressed = Vec::new();

        for part in parts {
            let part = part.trim();
            if part == "&self" || part == "&mut self" {
                compressed.push("&self".to_string());
            } else if part == "self" {
                compressed.push("self".to_string());
            } else if let Some((name, _)) = part.split_once(':') {
                let name = name.trim();
                if !name.is_empty() {
                    compressed.push(name.to_string());
                }
            }
        }

        compressed.join(", ")
    }

    /// TypeScript/TSX compression
    fn compress_typescript(content: &str) -> String {
        let mut output = String::new();
        let mut imports = Vec::new();

        // Extract imports
        for cap in patterns::TS_IMPORT.captures_iter(content) {
            let module = &cap[1];
            if !imports.contains(&module.to_string()) {
                imports.push(module.to_string());
            }
        }
        if !imports.is_empty() {
            output.push_str(&format!("imports: {}\n\n", imports.join(", ")));
        }

        // Extract interfaces with extends
        for cap in patterns::TS_INTERFACE.captures_iter(content) {
            let name = &cap[1];
            if let Some(extends) = cap.get(2) {
                output.push_str(&format!("interface {} extends {}\n", name, extends.as_str().trim()));
            } else {
                output.push_str(&format!("interface {}\n", name));
            }
        }

        // Extract type aliases with definition
        for cap in patterns::TS_TYPE.captures_iter(content) {
            let name = &cap[1];
            let def = Self::simplify_ts_type(&cap[2]);
            output.push_str(&format!("type {} = {}\n", name, def));
        }

        // Extract classes with inheritance
        for cap in patterns::TS_CLASS.captures_iter(content) {
            let name = &cap[1];
            let mut class_def = format!("class {}", name);
            
            if let Some(extends) = cap.get(2) {
                class_def.push_str(&format!(" extends {}", extends.as_str()));
            }
            if let Some(implements) = cap.get(3) {
                class_def.push_str(&format!(" implements {}", implements.as_str().trim()));
            }
            output.push_str(&format!("{}\n", class_def));
        }

        // Extract functions
        for cap in patterns::TS_FN.captures_iter(content) {
            let name = &cap[1];
            let params = Self::compress_ts_params(&cap[2]);
            let ret = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("void");
            output.push_str(&format!("  fn {}({}) -> {}\n", name, params, ret));
        }

        // Extract arrow functions
        for cap in patterns::TS_ARROW.captures_iter(content) {
            let name = &cap[1];
            let ftype = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if ftype.is_empty() {
                output.push_str(&format!("  const {} = () => ...\n", name));
            } else {
                output.push_str(&format!("  const {}: {}\n", name, Self::simplify_ts_type(ftype)));
            }
        }

        output
    }

    /// Simplify TypeScript type
    fn simplify_ts_type(t: &str) -> String {
        let t = t.trim();
        if t.len() > 50 {
            format!("{}...", &t[..47])
        } else {
            t.to_string()
        }
    }

    /// Compress TypeScript params
    fn compress_ts_params(params: &str) -> String {
        if params.trim().is_empty() {
            return String::new();
        }

        let parts: Vec<&str> = params.split(',').collect();
        let mut compressed = Vec::new();

        for part in parts {
            let part = part.trim();
            if let Some((name, _)) = part.split_once(':') {
                compressed.push(name.trim().to_string());
            } else {
                compressed.push(part.to_string());
            }
        }

        compressed.join(", ")
    }

    /// JavaScript compression
    fn compress_javascript(content: &str) -> String {
        let mut output = String::new();
        let mut imports = Vec::new();

        // Extract ES imports
        for cap in patterns::TS_IMPORT.captures_iter(content) {
            let module = &cap[1];
            if !imports.contains(&module.to_string()) {
                imports.push(module.to_string());
            }
        }

        // Extract CommonJS requires
        for cap in patterns::JS_REQUIRE.captures_iter(content) {
            let module = &cap[1];
            if !imports.contains(&module.to_string()) {
                imports.push(module.to_string());
            }
        }

        if !imports.is_empty() {
            output.push_str(&format!("imports: {}\n\n", imports.join(", ")));
        }

        // Extract classes
        for cap in patterns::TS_CLASS.captures_iter(content) {
            let name = &cap[1];
            if let Some(extends) = cap.get(2) {
                output.push_str(&format!("class {} extends {}\n", name, extends.as_str()));
            } else {
                output.push_str(&format!("class {}\n", name));
            }
        }

        // Extract functions
        for cap in patterns::TS_FN.captures_iter(content) {
            let name = &cap[1];
            let params = Self::compress_ts_params(&cap[2]);
            output.push_str(&format!("  fn {}({})\n", name, params));
        }

        // Extract arrow functions
        for cap in patterns::TS_ARROW.captures_iter(content) {
            output.push_str(&format!("  const {} = () => ...\n", &cap[1]));
        }

        output
    }

    /// Python compression with class methods
    fn compress_python(content: &str) -> String {
        let mut output = String::new();
        let mut imports = Vec::new();
        let mut current_class: Option<String> = None;

        // Extract imports
        for cap in patterns::PY_IMPORT.captures_iter(content) {
            if let Some(module) = cap.get(1) {
                let mod_name = module.as_str().split('.').next().unwrap_or("");
                if !mod_name.is_empty() && !imports.contains(&mod_name.to_string()) {
                    imports.push(mod_name.to_string());
                }
            } else if let Some(names) = cap.get(2) {
                let mod_name = names.as_str().split(',').next().unwrap_or("").trim();
                if !mod_name.is_empty() && !imports.contains(&mod_name.to_string()) {
                    imports.push(mod_name.to_string());
                }
            }
        }
        if !imports.is_empty() {
            output.push_str(&format!("imports: {}\n\n", imports.join(", ")));
        }

        // Process line by line to track class context
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Check for class definition
            if let Some(cap) = patterns::PY_CLASS.captures(trimmed) {
                let name = &cap[1];
                let bases = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                
                if bases.is_empty() {
                    output.push_str(&format!("class {}\n", name));
                } else {
                    output.push_str(&format!("class {}({})\n", name, bases));
                }
                current_class = Some(name.to_string());
            }
            
            // Check for function/method definition
            if let Some(cap) = patterns::PY_FN.captures(line) {
                let name = &cap[1];
                let params = Self::compress_python_params(&cap[2]);
                let ret = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("");
                
                let prefix = if current_class.is_some() && line.starts_with("    ") {
                    "  "
                } else {
                    current_class = None;
                    ""
                };
                
                if ret.is_empty() {
                    output.push_str(&format!("{}def {}({})\n", prefix, name, params));
                } else {
                    output.push_str(&format!("{}def {}({}) -> {}\n", prefix, name, params, ret));
                }
            }
        }

        output
    }

    /// Compress Python function parameters
    fn compress_python_params(params: &str) -> String {
        if params.trim().is_empty() {
            return String::new();
        }

        let parts: Vec<&str> = params.split(',').collect();
        let mut compressed = Vec::new();

        for part in parts {
            let part = part.trim();
            if part == "self" || part == "cls" {
                compressed.push(part.to_string());
            } else {
                let name = part
                    .split(':')
                    .next()
                    .unwrap_or(part)
                    .split('=')
                    .next()
                    .unwrap_or("")
                    .trim();
                
                if !name.is_empty() {
                    if part.starts_with("**") {
                        compressed.push(format!("**{}", name.trim_start_matches("**")));
                    } else if part.starts_with('*') {
                        compressed.push(format!("*{}", name.trim_start_matches('*')));
                    } else {
                        compressed.push(name.to_string());
                    }
                }
            }
        }

        compressed.join(", ")
    }

    /// Go compression
    fn compress_go(content: &str) -> String {
        let mut output = String::new();
        let mut imports = Vec::new();

        // Check for package declaration
        let re_package = Regex::new(r"(?m)^package\s+(\w+)").unwrap();
        if let Some(cap) = re_package.captures(content) {
            output.push_str(&format!("package {}\n\n", &cap[1]));
        }

        // Extract imports
        for cap in patterns::GO_IMPORT.captures_iter(content) {
            let path = &cap[1];
            let name = path.rsplit('/').next().unwrap_or(path);
            if !imports.contains(&name.to_string()) {
                imports.push(name.to_string());
            }
        }
        if !imports.is_empty() {
            output.push_str(&format!("imports: {}\n\n", imports.join(", ")));
        }

        // Extract structs
        for cap in patterns::GO_STRUCT.captures_iter(content) {
            let name = &cap[1];
            let fields = Self::extract_go_struct_fields(&cap[2]);
            if fields.is_empty() {
                output.push_str(&format!("type {} struct\n", name));
            } else {
                output.push_str(&format!("type {} struct {{ {} }}\n", name, fields.join(", ")));
            }
        }

        // Extract interfaces
        for cap in patterns::GO_INTERFACE.captures_iter(content) {
            output.push_str(&format!("type {} interface\n", &cap[1]));
        }

        // Extract functions and methods
        for cap in patterns::GO_FN.captures_iter(content) {
            let fn_name = &cap[3];
            let params = Self::compress_go_params(&cap[4]);
            
            // Get return type
            let ret = cap.get(5)
                .or(cap.get(6))
                .map(|m| m.as_str().trim())
                .unwrap_or("");
            
            // Check if it's a method (has receiver)
            if let Some(receiver_type) = cap.get(2) {
                if ret.is_empty() {
                    output.push_str(&format!("  func ({}) {}({})\n", receiver_type.as_str(), fn_name, params));
                } else {
                    output.push_str(&format!("  func ({}) {}({}) {}\n", receiver_type.as_str(), fn_name, params, ret));
                }
            } else if ret.is_empty() {
                output.push_str(&format!("func {}({})\n", fn_name, params));
            } else {
                output.push_str(&format!("func {}({}) {}\n", fn_name, params, ret));
            }
        }

        output
    }

    /// Extract Go struct fields
    fn extract_go_struct_fields(body: &str) -> Vec<String> {
        let mut fields = Vec::new();
        
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0];
                let ftype = parts[1];
                if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    fields.push(format!("{}: {}", name, Self::simplify_type(ftype)));
                }
            }
        }
        
        fields
    }

    /// Compress Go params
    fn compress_go_params(params: &str) -> String {
        if params.trim().is_empty() {
            return String::new();
        }

        let parts: Vec<&str> = params.split(',').collect();
        let mut compressed = Vec::new();

        for part in parts {
            let part = part.trim();
            let name = part.split_whitespace().next().unwrap_or(part);
            if !name.is_empty() {
                compressed.push(name.to_string());
            }
        }

        compressed.join(", ")
    }

    /// Elixir compression
    fn compress_elixir(content: &str) -> String {
        let mut output = String::new();
        let mut uses = Vec::new();
        let mut current_module: Option<String> = None;

        // Extract modules
        for cap in patterns::EX_MODULE.captures_iter(content) {
            let module = &cap[1];
            if current_module.is_some() {
                output.push('\n');
            }
            output.push_str(&format!("defmodule {}\n", module));
            current_module = Some(module.to_string());
        }

        // Extract uses/imports/aliases
        for cap in patterns::EX_USE.captures_iter(content) {
            let module = &cap[1];
            if !uses.contains(&module.to_string()) {
                uses.push(module.to_string());
            }
        }
        if !uses.is_empty() {
            output.push_str(&format!("  uses: {}\n", uses.join(", ")));
        }

        // Extract defstruct
        for cap in patterns::EX_STRUCT.captures_iter(content) {
            let fields: Vec<&str> = cap[1].split(',')
                .map(|s| s.trim().trim_start_matches(':'))
                .filter(|s| !s.is_empty())
                .collect();
            output.push_str(&format!("  defstruct [{}]\n", fields.join(", ")));
        }

        // Extract callbacks
        for cap in patterns::EX_CALLBACK.captures_iter(content) {
            let name = &cap[1];
            let params = Self::compress_elixir_params(&cap[2]);
            output.push_str(&format!("  @callback {}({})\n", name, params));
        }

        // Extract functions
        for cap in patterns::EX_FN.captures_iter(content) {
            let name = &cap[1];
            let params = Self::compress_elixir_params(&cap[2]);
            output.push_str(&format!("  def {}({})\n", name, params));
        }

        // Extract macros
        for cap in patterns::EX_MACRO.captures_iter(content) {
            let name = &cap[1];
            let params = Self::compress_elixir_params(&cap[2]);
            output.push_str(&format!("  defmacro {}({})\n", name, params));
        }

        output
    }

    /// Compress Elixir params
    fn compress_elixir_params(params: &str) -> String {
        if params.trim().is_empty() {
            return String::new();
        }

        params.split(',')
            .map(|p| {
                let p = p.trim();
                // Handle pattern matching like %User{} = user
                if p.contains('=') {
                    p.split('=').last().unwrap_or(p).trim()
                } else {
                    p
                }
            })
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Gleam compression
    fn compress_gleam(content: &str) -> String {
        let mut output = String::new();
        let mut imports = Vec::new();

        // Extract imports
        for cap in patterns::GLEAM_IMPORT.captures_iter(content) {
            let module = &cap[1];
            let name = module.rsplit('/').next().unwrap_or(module);
            if !imports.contains(&name.to_string()) {
                imports.push(name.to_string());
            }
        }
        if !imports.is_empty() {
            output.push_str(&format!("imports: {}\n\n", imports.join(", ")));
        }

        // Extract type definitions
        for cap in patterns::GLEAM_TYPE.captures_iter(content) {
            output.push_str(&format!("type {}\n", &cap[1]));
        }

        // Extract constants
        for cap in patterns::GLEAM_CONST.captures_iter(content) {
            let name = &cap[1];
            if let Some(ctype) = cap.get(2) {
                output.push_str(&format!("const {}: {}\n", name, ctype.as_str()));
            } else {
                output.push_str(&format!("const {}\n", name));
            }
        }

        // Extract functions
        for cap in patterns::GLEAM_FN.captures_iter(content) {
            let name = &cap[1];
            let params = Self::compress_gleam_params(&cap[2]);
            let ret = cap.get(3).map(|m| m.as_str()).unwrap_or("_");
            output.push_str(&format!("  fn {}({}) -> {}\n", name, params, ret));
        }

        output
    }

    /// Compress Gleam params
    fn compress_gleam_params(params: &str) -> String {
        if params.trim().is_empty() {
            return String::new();
        }

        params.split(',')
            .map(|p| {
                let p = p.trim();
                // Extract param name before ':'
                p.split(':').next().unwrap_or(p).trim()
            })
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Clojure compression
    fn compress_clojure(content: &str) -> String {
        let mut output = String::new();

        // Extract namespace
        for cap in patterns::CLJ_NS.captures_iter(content) {
            output.push_str(&format!("(ns {})\n", &cap[1]));
        }

        // Extract requires (simplified)
        let mut requires = Vec::new();
        for cap in patterns::CLJ_REQUIRE.captures_iter(content) {
            let req = &cap[1];
            // Get first part of each require
            for part in req.split_whitespace() {
                let name = part.trim_matches(|c| c == '[' || c == ']' || c == ':');
                if !name.is_empty() && !requires.contains(&name.to_string()) {
                    requires.push(name.to_string());
                    break;
                }
            }
        }
        if !requires.is_empty() {
            output.push_str(&format!("requires: {}\n\n", requires.join(", ")));
        }

        // Extract protocols
        for cap in patterns::CLJ_DEFPROTOCOL.captures_iter(content) {
            output.push_str(&format!("(defprotocol {})\n", &cap[1]));
        }

        // Extract records
        for cap in patterns::CLJ_DEFRECORD.captures_iter(content) {
            let name = &cap[1];
            let fields = cap[2].split_whitespace().collect::<Vec<_>>().join(" ");
            output.push_str(&format!("(defrecord {} [{}])\n", name, fields));
        }

        // Extract defs (constants/vars)
        for cap in patterns::CLJ_DEF.captures_iter(content) {
            let name = &cap[1];
            // Skip if it's actually a defn/defmacro/etc
            if !name.starts_with('(') {
                output.push_str(&format!("(def {})\n", name));
            }
        }

        // Extract functions
        for cap in patterns::CLJ_DEFN.captures_iter(content) {
            let name = &cap[1];
            let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            output.push_str(&format!("(defn {} [{}])\n", name, params));
        }

        // Extract macros
        for cap in patterns::CLJ_DEFMACRO.captures_iter(content) {
            let name = &cap[1];
            let params = &cap[2];
            output.push_str(&format!("(defmacro {} [{}])\n", name, params));
        }

        output
    }

    /// Java compression
    fn compress_java(content: &str) -> String {
        let mut output = String::new();
        let mut imports = Vec::new();

        // Extract package
        for cap in patterns::JAVA_PACKAGE.captures_iter(content) {
            output.push_str(&format!("package {}\n\n", &cap[1]));
        }

        // Extract imports (simplified to package level)
        for cap in patterns::JAVA_IMPORT.captures_iter(content) {
            let path = &cap[1];
            // Get package (first 2-3 parts)
            let parts: Vec<&str> = path.split('.').collect();
            let pkg = if parts.len() >= 2 {
                parts[..2.min(parts.len())].join(".")
            } else {
                path.to_string()
            };
            if !imports.contains(&pkg) {
                imports.push(pkg);
            }
        }
        if !imports.is_empty() {
            output.push_str(&format!("imports: {}\n\n", imports.join(", ")));
        }

        // Extract records (Java 16+)
        for cap in patterns::JAVA_RECORD.captures_iter(content) {
            let name = &cap[1];
            let params = Self::compress_java_params(&cap[2]);
            output.push_str(&format!("record {}({})\n", name, params));
        }

        // Extract enums
        for cap in patterns::JAVA_ENUM.captures_iter(content) {
            let name = &cap[1];
            if let Some(implements) = cap.get(2) {
                output.push_str(&format!("enum {} implements {}\n", name, implements.as_str().trim()));
            } else {
                output.push_str(&format!("enum {}\n", name));
            }
        }

        // Extract interfaces
        for cap in patterns::JAVA_INTERFACE.captures_iter(content) {
            let name = &cap[1];
            if let Some(extends) = cap.get(2) {
                output.push_str(&format!("interface {} extends {}\n", name, extends.as_str().trim()));
            } else {
                output.push_str(&format!("interface {}\n", name));
            }
        }

        // Extract classes
        for cap in patterns::JAVA_CLASS.captures_iter(content) {
            let name = &cap[1];
            let mut class_def = format!("class {}", name);
            
            if let Some(extends) = cap.get(2) {
                class_def.push_str(&format!(" extends {}", extends.as_str()));
            }
            if let Some(implements) = cap.get(3) {
                class_def.push_str(&format!(" implements {}", implements.as_str().trim()));
            }
            output.push_str(&format!("{}\n", class_def));
        }

        // Extract methods
        for cap in patterns::JAVA_METHOD.captures_iter(content) {
            let ret_type = &cap[1];
            let name = &cap[2];
            let params = Self::compress_java_params(&cap[3]);
            
            // Skip constructors (return type = class name pattern)
            if ret_type.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) || ret_type.contains('<') {
                output.push_str(&format!("  {} {}({})\n", Self::simplify_type(ret_type), name, params));
            }
        }

        output
    }

    /// Compress Java params
    fn compress_java_params(params: &str) -> String {
        if params.trim().is_empty() {
            return String::new();
        }

        params.split(',')
            .map(|p| {
                let p = p.trim();
                // Java params are "Type name" - get the name (last word)
                p.split_whitespace().last().unwrap_or(p)
            })
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Bash/Shell compression
    fn compress_bash(content: &str) -> String {
        let mut output = String::new();
        let mut sources = Vec::new();
        let mut exports = Vec::new();
        let mut aliases = Vec::new();
        let mut functions = Vec::new();

        // Check for shebang
        if let Some(first_line) = content.lines().next() {
            if first_line.starts_with("#!") {
                let shell = first_line.rsplit('/').next().unwrap_or("sh");
                output.push_str(&format!("#!/{}\n", shell));
            }
        }

        // Extract sourced files
        for cap in patterns::BASH_SOURCE.captures_iter(content) {
            let path = &cap[1];
            let name = path.rsplit('/').next().unwrap_or(path);
            if !sources.contains(&name.to_string()) {
                sources.push(name.to_string());
            }
        }
        if !sources.is_empty() {
            output.push_str(&format!("sources: {}\n", sources.join(", ")));
        }

        // Extract exports
        for cap in patterns::BASH_EXPORT.captures_iter(content) {
            exports.push(cap[1].to_string());
        }
        if !exports.is_empty() {
            output.push_str(&format!("exports: {}\n", exports.join(", ")));
        }

        // Extract aliases
        for cap in patterns::BASH_ALIAS.captures_iter(content) {
            aliases.push(cap[1].to_string());
        }
        if !aliases.is_empty() {
            output.push_str(&format!("aliases: {}\n", aliases.join(", ")));
        }

        // Extract functions
        for cap in patterns::BASH_FN.captures_iter(content) {
            functions.push(cap[1].to_string());
        }
        if !functions.is_empty() {
            output.push('\n');
            for func in functions {
                output.push_str(&format!("{}()\n", func));
            }
        }

        output
    }

    /// Markdown compression (headings and code blocks)
    fn compress_markdown(content: &str) -> String {
        let mut output = String::new();

        for cap in patterns::MD_HEADING.captures_iter(content) {
            let level = cap[1].len();
            let text = &cap[2];
            output.push_str(&format!("{} {}\n", "#".repeat(level), text));
        }

        // Count code blocks by language
        let mut code_blocks: Vec<String> = Vec::new();
        for cap in patterns::MD_CODE_BLOCK.captures_iter(content) {
            if let Some(lang) = cap.get(1) {
                let lang = lang.as_str();
                if !code_blocks.contains(&lang.to_string()) {
                    code_blocks.push(lang.to_string());
                }
            }
        }
        if !code_blocks.is_empty() {
            output.push_str(&format!("\ncode blocks: {}\n", code_blocks.join(", ")));
        }

        output
    }

    /// JSON compression (top-level keys)
    fn compress_json(content: &str) -> String {
        let mut output = String::new();
        
        // Try to parse as JSON and extract top-level keys
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(obj) = val.as_object() {
                let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
                output.push_str(&format!("keys: {}\n", keys.join(", ")));
                
                // Show nested structure for important keys
                for (key, value) in obj.iter() {
                    if value.is_object() {
                        if let Some(nested) = value.as_object() {
                            let nested_keys: Vec<&str> = nested.keys().take(5).map(|k| k.as_str()).collect();
                            output.push_str(&format!("  {}: {{ {} }}\n", key, nested_keys.join(", ")));
                        }
                    } else if value.is_array() {
                        if let Some(arr) = value.as_array() {
                            output.push_str(&format!("  {}: [{} items]\n", key, arr.len()));
                        }
                    }
                }
            } else if let Some(arr) = val.as_array() {
                output.push_str(&format!("[{} items]\n", arr.len()));
            }
        } else {
            output.push_str("(invalid JSON)\n");
        }
        
        output
    }

    /// YAML compression (top-level keys)
    fn compress_yaml(content: &str) -> String {
        let mut output = String::new();
        let re_key = Regex::new(r"(?m)^(\w+):").unwrap();
        
        let mut keys = Vec::new();
        for cap in re_key.captures_iter(content) {
            let key = &cap[1];
            if !keys.contains(&key.to_string()) {
                keys.push(key.to_string());
            }
        }
        
        if !keys.is_empty() {
            output.push_str(&format!("keys: {}\n", keys.join(", ")));
        }
        
        output
    }

    /// TOML compression (sections and keys)
    fn compress_toml(content: &str) -> String {
        let mut output = String::new();
        let re_section = Regex::new(r"(?m)^\[([^\]]+)\]").unwrap();
        let re_key = Regex::new(r"(?m)^(\w+)\s*=").unwrap();
        
        let mut sections = Vec::new();
        for cap in re_section.captures_iter(content) {
            sections.push(cap[1].to_string());
        }
        
        if !sections.is_empty() {
            output.push_str(&format!("sections: {}\n", sections.join(", ")));
        }
        
        // Get root-level keys
        let first_section = content.find('[').unwrap_or(content.len());
        let root = &content[..first_section];
        let mut keys = Vec::new();
        for cap in re_key.captures_iter(root) {
            keys.push(cap[1].to_string());
        }
        if !keys.is_empty() {
            output.push_str(&format!("root keys: {}\n", keys.join(", ")));
        }
        
        output
    }

    /// Zig compression
    fn compress_zig(content: &str) -> String {
        let mut output = String::new();
        let mut imports = Vec::new();

        // Extract imports
        for cap in patterns::ZIG_IMPORT.captures_iter(content) {
            let module = &cap[1];
            if !imports.contains(&module.to_string()) {
                imports.push(module.to_string());
            }
        }
        if !imports.is_empty() {
            output.push_str(&format!("imports: {}\n\n", imports.join(", ")));
        }

        // Extract structs
        for cap in patterns::ZIG_STRUCT.captures_iter(content) {
            output.push_str(&format!("struct {}\n", &cap[1]));
        }

        // Extract enums
        for cap in patterns::ZIG_ENUM.captures_iter(content) {
            output.push_str(&format!("enum {}\n", &cap[1]));
        }

        // Extract functions
        for cap in patterns::ZIG_FN.captures_iter(content) {
            let name = &cap[1];
            let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let ret = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("");
            if ret.is_empty() {
                output.push_str(&format!("fn {}({})\n", name, params));
            } else {
                output.push_str(&format!("fn {}({}) {}\n", name, params, ret));
            }
        }

        output
    }

    /// C compression  
    fn compress_c(content: &str) -> String {
        let mut output = String::new();
        let mut includes = Vec::new();
        let mut defines = Vec::new();

        // Extract includes
        for cap in patterns::C_INCLUDE.captures_iter(content) {
            let header = &cap[1];
            if !includes.contains(&header.to_string()) {
                includes.push(header.to_string());
            }
        }
        if !includes.is_empty() {
            output.push_str(&format!("#include: {}\n", includes.join(", ")));
        }

        // Extract defines
        for cap in patterns::C_DEFINE.captures_iter(content) {
            defines.push(cap[1].to_string());
        }
        if !defines.is_empty() {
            output.push_str(&format!("#define: {}\n\n", defines.join(", ")));
        }

        // Extract structs
        for cap in patterns::C_STRUCT.captures_iter(content) {
            output.push_str(&format!("struct {}\n", &cap[1]));
        }

        // Extract enums
        for cap in patterns::C_ENUM.captures_iter(content) {
            output.push_str(&format!("enum {}\n", &cap[1]));
        }

        // Extract functions
        for cap in patterns::C_FN.captures_iter(content) {
            let ret = &cap[1];
            let name = &cap[2];
            let params = &cap[3];
            output.push_str(&format!("{} {}({})\n", ret.trim(), name, params));
        }

        output
    }

    /// C++ compression
    fn compress_cpp(content: &str) -> String {
        let mut output = Self::compress_c(content);
        
        // Extract namespaces
        for cap in patterns::CPP_NAMESPACE.captures_iter(content) {
            output.push_str(&format!("namespace {}\n", &cap[1]));
        }

        // Extract classes
        for cap in patterns::CPP_CLASS.captures_iter(content) {
            let name = &cap[1];
            if let Some(base) = cap.get(2) {
                output.push_str(&format!("class {} : {}\n", name, base.as_str()));
            } else {
                output.push_str(&format!("class {}\n", name));
            }
        }

        output
    }

    /// SQL compression
    fn compress_sql(content: &str) -> String {
        let mut output = String::new();
        let mut tables = Vec::new();
        let mut indexes = Vec::new();
        let mut views = Vec::new();
        let mut functions = Vec::new();

        for cap in patterns::SQL_CREATE_TABLE.captures_iter(content) {
            tables.push(cap[1].to_string());
        }
        for cap in patterns::SQL_CREATE_INDEX.captures_iter(content) {
            indexes.push(cap[1].to_string());
        }
        for cap in patterns::SQL_CREATE_VIEW.captures_iter(content) {
            views.push(cap[1].to_string());
        }
        for cap in patterns::SQL_CREATE_FUNCTION.captures_iter(content) {
            functions.push(cap[1].to_string());
        }

        if !tables.is_empty() {
            output.push_str(&format!("tables: {}\n", tables.join(", ")));
        }
        if !indexes.is_empty() {
            output.push_str(&format!("indexes: {}\n", indexes.join(", ")));
        }
        if !views.is_empty() {
            output.push_str(&format!("views: {}\n", views.join(", ")));
        }
        if !functions.is_empty() {
            output.push_str(&format!("functions: {}\n", functions.join(", ")));
        }

        output
    }

    /// CSS compression
    fn compress_css(content: &str) -> String {
        let mut output = String::new();
        let mut selectors = Vec::new();
        let mut variables = Vec::new();
        let mut keyframes = Vec::new();
        let mut medias = Vec::new();

        for cap in patterns::CSS_VARIABLE.captures_iter(content) {
            variables.push(format!("--{}", &cap[1]));
        }
        for cap in patterns::CSS_KEYFRAMES.captures_iter(content) {
            keyframes.push(cap[1].to_string());
        }
        for cap in patterns::CSS_MEDIA.captures_iter(content) {
            let query = cap[1].trim();
            if query.len() < 50 && !medias.contains(&query.to_string()) {
                medias.push(query.to_string());
            }
        }
        for cap in patterns::CSS_SELECTOR.captures_iter(content) {
            let sel = &cap[1];
            if sel.starts_with('.') || sel.starts_with('#') {
                if selectors.len() < 20 {
                    selectors.push(sel.to_string());
                }
            }
        }

        if !variables.is_empty() {
            output.push_str(&format!("variables: {}\n", variables.join(", ")));
        }
        if !keyframes.is_empty() {
            output.push_str(&format!("keyframes: {}\n", keyframes.join(", ")));
        }
        if !medias.is_empty() {
            output.push_str(&format!("media: {}\n", medias.join(" | ")));
        }
        if !selectors.is_empty() {
            output.push_str(&format!("selectors: {}\n", selectors.join(", ")));
        }

        output
    }

    /// Dockerfile compression
    fn compress_dockerfile(content: &str) -> String {
        let mut output = String::new();
        let mut stages = Vec::new();
        let mut ports = Vec::new();
        let mut envs = Vec::new();

        for cap in patterns::DOCKER_FROM.captures_iter(content) {
            stages.push(cap[1].to_string());
        }
        for cap in patterns::DOCKER_EXPOSE.captures_iter(content) {
            ports.push(cap[1].to_string());
        }
        for cap in patterns::DOCKER_ENV.captures_iter(content) {
            envs.push(cap[1].to_string());
        }

        if !stages.is_empty() {
            output.push_str(&format!("FROM: {}\n", stages.join(" -> ")));
        }
        if !ports.is_empty() {
            output.push_str(&format!("EXPOSE: {}\n", ports.join(", ")));
        }
        if !envs.is_empty() {
            output.push_str(&format!("ENV: {}\n", envs.join(", ")));
        }

        // Get CMD/ENTRYPOINT
        for cap in patterns::DOCKER_CMD.captures_iter(content) {
            output.push_str(&format!("CMD: {}\n", cap[1].trim()));
        }

        output
    }

    /// Swift compression
    fn compress_swift(content: &str) -> String {
        let mut output = String::new();
        let mut imports = Vec::new();

        for cap in patterns::SWIFT_IMPORT.captures_iter(content) {
            imports.push(cap[1].to_string());
        }
        if !imports.is_empty() {
            output.push_str(&format!("import: {}\n\n", imports.join(", ")));
        }

        // Protocols
        for cap in patterns::SWIFT_PROTOCOL.captures_iter(content) {
            let name = &cap[1];
            if let Some(conforms) = cap.get(2) {
                output.push_str(&format!("protocol {}: {}\n", name, conforms.as_str().trim()));
            } else {
                output.push_str(&format!("protocol {}\n", name));
            }
        }

        // Classes
        for cap in patterns::SWIFT_CLASS.captures_iter(content) {
            let name = &cap[1];
            if let Some(inherits) = cap.get(2) {
                output.push_str(&format!("class {}: {}\n", name, inherits.as_str().trim()));
            } else {
                output.push_str(&format!("class {}\n", name));
            }
        }

        // Structs
        for cap in patterns::SWIFT_STRUCT.captures_iter(content) {
            let name = &cap[1];
            if let Some(conforms) = cap.get(2) {
                output.push_str(&format!("struct {}: {}\n", name, conforms.as_str().trim()));
            } else {
                output.push_str(&format!("struct {}\n", name));
            }
        }

        // Enums
        for cap in patterns::SWIFT_ENUM.captures_iter(content) {
            output.push_str(&format!("enum {}\n", &cap[1]));
        }

        // Extensions
        for cap in patterns::SWIFT_EXTENSION.captures_iter(content) {
            let name = &cap[1];
            if let Some(conforms) = cap.get(2) {
                output.push_str(&format!("extension {}: {}\n", name, conforms.as_str().trim()));
            } else {
                output.push_str(&format!("extension {}\n", name));
            }
        }

        // Functions
        for cap in patterns::SWIFT_FUNC.captures_iter(content) {
            let name = &cap[1];
            let params = &cap[2];
            if let Some(ret) = cap.get(3) {
                output.push_str(&format!("  func {}({}) -> {}\n", name, params, ret.as_str().trim()));
            } else {
                output.push_str(&format!("  func {}({})\n", name, params));
            }
        }

        output
    }

    /// Kotlin compression
    fn compress_kotlin(content: &str) -> String {
        let mut output = String::new();

        // Package
        for cap in patterns::KOTLIN_PACKAGE.captures_iter(content) {
            output.push_str(&format!("package {}\n\n", &cap[1]));
        }

        // Interfaces
        for cap in patterns::KOTLIN_INTERFACE.captures_iter(content) {
            let name = &cap[1];
            if let Some(extends) = cap.get(2) {
                output.push_str(&format!("interface {}: {}\n", name, extends.as_str().trim()));
            } else {
                output.push_str(&format!("interface {}\n", name));
            }
        }

        // Classes
        for cap in patterns::KOTLIN_CLASS.captures_iter(content) {
            let name = &cap[1];
            if let Some(extends) = cap.get(2) {
                output.push_str(&format!("class {}: {}\n", name, extends.as_str().trim()));
            } else {
                output.push_str(&format!("class {}\n", name));
            }
        }

        // Objects
        for cap in patterns::KOTLIN_OBJECT.captures_iter(content) {
            if let Some(name) = cap.get(1) {
                output.push_str(&format!("object {}\n", name.as_str()));
            }
        }

        // Functions
        for cap in patterns::KOTLIN_FUN.captures_iter(content) {
            let name = &cap[1];
            let params = &cap[2];
            if let Some(ret) = cap.get(3) {
                output.push_str(&format!("  fun {}({}): {}\n", name, params, ret.as_str().trim()));
            } else {
                output.push_str(&format!("  fun {}({})\n", name, params));
            }
        }

        output
    }

    /// Scala compression (similar to Kotlin)
    fn compress_scala(content: &str) -> String {
        // Reuse Kotlin patterns as Scala is similar
        Self::compress_kotlin(content)
    }

    /// Ruby compression
    fn compress_ruby(content: &str) -> String {
        let mut output = String::new();
        let mut requires = Vec::new();

        for cap in patterns::RUBY_REQUIRE.captures_iter(content) {
            requires.push(cap[1].to_string());
        }
        if !requires.is_empty() {
            output.push_str(&format!("require: {}\n\n", requires.join(", ")));
        }

        // Modules
        for cap in patterns::RUBY_MODULE.captures_iter(content) {
            output.push_str(&format!("module {}\n", &cap[1]));
        }

        // Classes
        for cap in patterns::RUBY_CLASS.captures_iter(content) {
            let name = &cap[1];
            if let Some(parent) = cap.get(2) {
                output.push_str(&format!("class {} < {}\n", name, parent.as_str()));
            } else {
                output.push_str(&format!("class {}\n", name));
            }
        }

        // Methods
        for cap in patterns::RUBY_DEF.captures_iter(content) {
            let name = &cap[1];
            let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            output.push_str(&format!("  def {}({})\n", name, params));
        }

        output
    }

    /// PHP compression
    fn compress_php(content: &str) -> String {
        let mut output = String::new();

        // Namespace
        for cap in patterns::PHP_NAMESPACE.captures_iter(content) {
            output.push_str(&format!("namespace {}\n\n", &cap[1]));
        }

        // Traits
        for cap in patterns::PHP_TRAIT.captures_iter(content) {
            output.push_str(&format!("trait {}\n", &cap[1]));
        }

        // Interfaces
        for cap in patterns::PHP_INTERFACE.captures_iter(content) {
            let name = &cap[1];
            if let Some(extends) = cap.get(2) {
                output.push_str(&format!("interface {} extends {}\n", name, extends.as_str().trim()));
            } else {
                output.push_str(&format!("interface {}\n", name));
            }
        }

        // Classes
        for cap in patterns::PHP_CLASS.captures_iter(content) {
            let name = &cap[1];
            let mut class_def = format!("class {}", name);
            if let Some(extends) = cap.get(2) {
                class_def.push_str(&format!(" extends {}", extends.as_str()));
            }
            if let Some(implements) = cap.get(3) {
                class_def.push_str(&format!(" implements {}", implements.as_str().trim()));
            }
            output.push_str(&format!("{}\n", class_def));
        }

        // Functions
        for cap in patterns::PHP_FUNCTION.captures_iter(content) {
            let name = &cap[1];
            let params = &cap[2];
            if let Some(ret) = cap.get(3) {
                output.push_str(&format!("  function {}({}): {}\n", name, params, ret.as_str()));
            } else {
                output.push_str(&format!("  function {}({})\n", name, params));
            }
        }

        output
    }

    /// Lua compression
    fn compress_lua(content: &str) -> String {
        let mut output = String::new();
        let mut requires = Vec::new();

        for cap in patterns::LUA_REQUIRE.captures_iter(content) {
            requires.push(cap[1].to_string());
        }
        if !requires.is_empty() {
            output.push_str(&format!("require: {}\n\n", requires.join(", ")));
        }

        // Global functions
        for cap in patterns::LUA_FUNCTION.captures_iter(content) {
            let name = &cap[1];
            let params = &cap[2];
            output.push_str(&format!("function {}({})\n", name, params));
        }

        // Local functions
        for cap in patterns::LUA_LOCAL_FN.captures_iter(content) {
            let name = &cap[1];
            let params = &cap[2];
            output.push_str(&format!("local {}({})\n", name, params));
        }

        // Tables
        for cap in patterns::LUA_TABLE.captures_iter(content) {
            output.push_str(&format!("table {}\n", &cap[1]));
        }

        output
    }

    /// Vue SFC compression
    fn compress_vue(content: &str) -> String {
        let mut output = String::from("<!-- Vue SFC -->\n");
        
        // Extract script content and compress as JS/TS
        if let Some(cap) = patterns::SFC_SCRIPT.captures(content) {
            let script = &cap[1];
            output.push_str("<script>\n");
            output.push_str(&Self::compress_typescript(script));
            output.push_str("</script>\n");
        }

        // Check for defineProps, defineEmits etc
        for cap in patterns::VUE_DEFINE.captures_iter(content) {
            let define_type = &cap[1];
            if let Some(types) = cap.get(2) {
                output.push_str(&format!("  define{}<{}>\n", define_type, types.as_str()));
            } else {
                output.push_str(&format!("  define{}\n", define_type));
            }
        }

        // Check for style lang
        for cap in patterns::SFC_STYLE.captures_iter(content) {
            if let Some(lang) = cap.get(1) {
                output.push_str(&format!("<style lang=\"{}\">\n", lang.as_str()));
            }
        }

        // Extract components used
        let mut components = Vec::new();
        for cap in patterns::SFC_COMPONENT.captures_iter(content) {
            let comp = &cap[1];
            if !components.contains(&comp.to_string()) && components.len() < 10 {
                components.push(comp.to_string());
            }
        }
        if !components.is_empty() {
            output.push_str(&format!("components: {}\n", components.join(", ")));
        }

        output
    }

    /// Svelte compression
    fn compress_svelte(content: &str) -> String {
        let mut output = String::from("<!-- Svelte -->\n");
        
        // Extract script
        if let Some(cap) = patterns::SFC_SCRIPT.captures(content) {
            let script = &cap[1];
            output.push_str("<script>\n");
            output.push_str(&Self::compress_typescript(script));
            output.push_str("</script>\n");
        }

        // Components
        let mut components = Vec::new();
        for cap in patterns::SFC_COMPONENT.captures_iter(content) {
            let comp = &cap[1];
            if !components.contains(&comp.to_string()) && components.len() < 10 {
                components.push(comp.to_string());
            }
        }
        if !components.is_empty() {
            output.push_str(&format!("components: {}\n", components.join(", ")));
        }

        output
    }

    /// Astro compression
    fn compress_astro(content: &str) -> String {
        let mut output = String::from("---\n");
        
        // Astro frontmatter is between ---
        if let Some(start) = content.find("---") {
            if let Some(end) = content[start + 3..].find("---") {
                let frontmatter = &content[start + 3..start + 3 + end];
                output.push_str(&Self::compress_typescript(frontmatter));
            }
        }
        output.push_str("---\n");

        // Components
        let mut components = Vec::new();
        for cap in patterns::SFC_COMPONENT.captures_iter(content) {
            let comp = &cap[1];
            if !components.contains(&comp.to_string()) && components.len() < 10 {
                components.push(comp.to_string());
            }
        }
        if !components.is_empty() {
            output.push_str(&format!("components: {}\n", components.join(", ")));
        }

        output
    }

    /// GraphQL compression
    fn compress_graphql(content: &str) -> String {
        let mut output = String::new();

        // Types
        for cap in patterns::GQL_TYPE.captures_iter(content) {
            let name = &cap[1];
            if let Some(implements) = cap.get(2) {
                output.push_str(&format!("type {} implements {}\n", name, implements.as_str().trim()));
            } else {
                output.push_str(&format!("type {}\n", name));
            }
        }

        // Interfaces
        for cap in patterns::GQL_INTERFACE.captures_iter(content) {
            output.push_str(&format!("interface {}\n", &cap[1]));
        }

        // Inputs
        for cap in patterns::GQL_INPUT.captures_iter(content) {
            output.push_str(&format!("input {}\n", &cap[1]));
        }

        // Enums
        for cap in patterns::GQL_ENUM.captures_iter(content) {
            output.push_str(&format!("enum {}\n", &cap[1]));
        }

        output
    }

    /// Terraform/HCL compression  
    fn compress_terraform(content: &str) -> String {
        let mut output = String::new();
        let mut resources = Vec::new();
        let mut data_sources = Vec::new();
        let mut variables = Vec::new();
        let mut outputs = Vec::new();
        let mut modules = Vec::new();

        for cap in patterns::TF_RESOURCE.captures_iter(content) {
            resources.push(format!("{}.{}", &cap[1], &cap[2]));
        }
        for cap in patterns::TF_DATA.captures_iter(content) {
            data_sources.push(format!("{}.{}", &cap[1], &cap[2]));
        }
        for cap in patterns::TF_VARIABLE.captures_iter(content) {
            variables.push(cap[1].to_string());
        }
        for cap in patterns::TF_OUTPUT.captures_iter(content) {
            outputs.push(cap[1].to_string());
        }
        for cap in patterns::TF_MODULE.captures_iter(content) {
            modules.push(cap[1].to_string());
        }

        if !resources.is_empty() {
            output.push_str(&format!("resources: {}\n", resources.join(", ")));
        }
        if !data_sources.is_empty() {
            output.push_str(&format!("data: {}\n", data_sources.join(", ")));
        }
        if !modules.is_empty() {
            output.push_str(&format!("modules: {}\n", modules.join(", ")));
        }
        if !variables.is_empty() {
            output.push_str(&format!("variables: {}\n", variables.join(", ")));
        }
        if !outputs.is_empty() {
            output.push_str(&format!("outputs: {}\n", outputs.join(", ")));
        }

        output
    }

    /// Generic compression for unknown file types
    fn compress_generic(content: &str) -> String {
        let mut output = String::new();
        let mut count = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                output.push_str(trimmed);
                output.push('\n');
                count += 1;
                if count >= 30 {
                    output.push_str(&format!("... ({} more lines)\n", content.lines().count() - 30));
                    break;
                }
            }
        }

        output
    }


    /// Get extension from path
    pub fn extension_from_path(path: &str) -> &str {
        path.rsplit('.').next().unwrap_or("")
    }

    /// Calculate compression ratio
    pub fn compression_ratio(original: &str, compressed: &str) -> f64 {
        if original.is_empty() {
            return 0.0;
        }
        let original_tokens = original.len() / 4;
        let compressed_tokens = compressed.len() / 4;
        100.0 - (compressed_tokens as f64 / original_tokens as f64 * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_rust_struct_with_fields() {
        let content = r#"
pub struct AuthService {
    db: Database,
    jwt: JwtEncoder,
    rate_limiter: RateLimiter,
}
        "#;
        let compressed = SymbolicCompressor::compress_rust(content);
        assert!(compressed.contains("struct AuthService"));
        assert!(compressed.contains("db:"));
        assert!(compressed.contains("jwt:"));
        println!("Compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_rust_enum_variants() {
        let content = r#"
pub enum AuthError {
    InvalidCredentials,
    RateLimited,
    TokenExpired { at: DateTime },
    DatabaseError(String),
}
        "#;
        let compressed = SymbolicCompressor::compress_rust(content);
        assert!(compressed.contains("enum AuthError"));
        assert!(compressed.contains("InvalidCredentials"));
        assert!(compressed.contains("RateLimited"));
        println!("Compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_rust_fn() {
        let content = r#"
pub async fn login(&self, email: &str, password: &str) -> Result<Token, AuthError> {
    // implementation...
}
        "#;
        let compressed = SymbolicCompressor::compress_rust(content);
        assert!(compressed.contains("fn login"));
        assert!(compressed.contains("&self"));
        assert!(compressed.contains("email"));
        assert!(compressed.contains("Result<Token, AuthError>"));
    }

    #[test]
    fn test_compress_go() {
        let content = r#"
package auth

import "database/sql"

type User struct {
    ID   int
    Name string
}

func (u *User) GetName() string {
    return u.Name
}

func NewUser(id int, name string) *User {
    return &User{ID: id, Name: name}
}
        "#;
        let compressed = SymbolicCompressor::compress_go(content);
        assert!(compressed.contains("package auth"));
        assert!(compressed.contains("type User struct"));
        assert!(compressed.contains("func (User) GetName"));
        assert!(compressed.contains("func NewUser"));
        println!("Go compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_typescript() {
        let content = r#"
import { Database } from './db';
import axios from 'axios';

export interface User {
    id: string;
    name: string;
}

export type UserRole = 'admin' | 'user' | 'guest';

export class UserService {
    constructor(private db: Database) {}
    
    async getUser(id: string): Promise<User> {
        return this.db.find(id);
    }
}
        "#;
        let compressed = SymbolicCompressor::compress_typescript(content);
        assert!(compressed.contains("imports:"));
        assert!(compressed.contains("interface User"));
        assert!(compressed.contains("type UserRole"));
        assert!(compressed.contains("class UserService"));
        println!("TS compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_python() {
        let content = r#"
from typing import Optional
import asyncio

class UserService:
    def __init__(self, db: Database) -> None:
        self.db = db
    
    async def get_user(self, user_id: str) -> Optional[User]:
        return await self.db.find(user_id)
    
    def create_user(self, name: str, email: str) -> User:
        return User(name=name, email=email)

def helper_function(x: int, y: int) -> int:
    return x + y
        "#;
        let compressed = SymbolicCompressor::compress_python(content);
        assert!(compressed.contains("class UserService"));
        assert!(compressed.contains("def __init__"));
        assert!(compressed.contains("def get_user"));
        assert!(compressed.contains("def helper_function"));
        println!("Python compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_json() {
        let content = r#"
{
    "name": "my-package",
    "version": "1.0.0",
    "dependencies": {
        "lodash": "^4.17.21",
        "axios": "^1.0.0"
    },
    "scripts": {
        "build": "tsc",
        "test": "jest"
    }
}
        "#;
        let compressed = SymbolicCompressor::compress(content, "json");
        assert!(compressed.contains("keys:"));
        assert!(compressed.contains("name"));
        assert!(compressed.contains("dependencies"));
        println!("JSON compressed:\n{}", compressed);
    }

    #[test]
    fn test_compression_ratio() {
        let full_code = r#"
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User authentication service
/// Handles all authentication-related operations
#[derive(Debug, Clone)]
pub struct AuthService {
    /// Database connection pool
    db: Database,
    /// JWT encoder for token generation
    jwt: JwtEncoder,
}

impl AuthService {
    /// Create a new AuthService instance
    pub fn new(db: Database, jwt: JwtEncoder) -> Self {
        Self { db, jwt }
    }
    
    /// Authenticate user with email and password
    pub async fn login(&self, email: &str, password: &str) -> Result<Token, AuthError> {
        let user = self.db.find_user_by_email(email).await?;
        if !verify_password(&user.password_hash, password) {
            return Err(AuthError::InvalidCredentials);
        }
        Ok(self.jwt.encode(&user)?)
    }
}
        "#;

        let compressed = SymbolicCompressor::compress_rust(full_code);
        let ratio = SymbolicCompressor::compression_ratio(full_code, &compressed);
        
        println!("Original: {} chars", full_code.len());
        println!("Compressed: {} chars", compressed.len());
        println!("Reduction: {:.1}%", ratio);
        println!("\nCompressed output:\n{}", compressed);
        
        assert!(ratio > 70.0, "Expected >70% reduction, got {:.1}%", ratio);
    }

    #[test]
    fn test_compress_nodes() {
        let nodes = vec![
            GraphNode {
                id: "struct:User".to_string(),
                content: "User struct".to_string(),
                signature: "User { id, name }".to_string(),
                node_type: "struct".to_string(),
                path: "src/models.rs".to_string(),
                edges: vec!["struct:Database".to_string()],
            },
            GraphNode {
                id: "fn:login".to_string(),
                content: "login function".to_string(),
                signature: "login(email, password) -> Token".to_string(),
                node_type: "fn".to_string(),
                path: "src/models.rs".to_string(),
                edges: vec![],
            },
        ];
        
        let compressed = SymbolicCompressor::compress_nodes(&nodes);
        assert!(compressed.contains("## src/models.rs"));
        assert!(compressed.contains("struct User"));
        assert!(compressed.contains("fn login"));
        println!("Nodes compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_elixir() {
        let content = r#"
defmodule MyApp.UserService do
  use GenServer
  alias MyApp.Repo
  
  defstruct [:id, :name, :email]

  @callback start_link(opts :: keyword()) :: GenServer.on_start()

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts)
  end

  defp validate_user(user) do
    # private function
  end

  defmacro my_macro(expr) do
    quote do: unquote(expr)
  end
end
        "#;
        let compressed = SymbolicCompressor::compress(content, "ex");
        assert!(compressed.contains("defmodule MyApp.UserService"));
        assert!(compressed.contains("defstruct"));
        assert!(compressed.contains("def start_link"));
        assert!(compressed.contains("defmacro my_macro"));
        println!("Elixir compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_gleam() {
        let content = r#"
import gleam/io
import gleam/list

pub type User {
  User(id: Int, name: String)
}

pub const max_users: Int = 100

pub fn greet(name: String) -> String {
  "Hello, " <> name
}

fn helper(x: Int) -> Int {
  x + 1
}
        "#;
        let compressed = SymbolicCompressor::compress(content, "gleam");
        assert!(compressed.contains("imports:"));
        assert!(compressed.contains("type User"));
        assert!(compressed.contains("const max_users"));
        assert!(compressed.contains("fn greet"));
        println!("Gleam compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_clojure() {
        let content = r#"
(ns myapp.core
  (:require [clojure.string :as str]
            [myapp.db :as db]))

(def config {:port 3000})

(defprotocol UserService
  (find-user [this id])
  (save-user [this user]))

(defrecord User [id name email])

(defn process-user [user opts]
  (println user))

(defn- private-helper [x]
  (* x 2))

(defmacro with-timing [body]
  `(time ~body))
        "#;
        let compressed = SymbolicCompressor::compress(content, "clj");
        assert!(compressed.contains("(ns myapp.core)"));
        assert!(compressed.contains("(defprotocol UserService)"));
        assert!(compressed.contains("(defrecord User"));
        assert!(compressed.contains("(defn process-user"));
        assert!(compressed.contains("(defmacro with-timing"));
        println!("Clojure compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_java() {
        let content = r#"
package com.example.app;

import java.util.List;
import java.util.Optional;
import com.example.db.Database;

public interface UserRepository {
    Optional<User> findById(Long id);
}

public enum UserRole {
    ADMIN, USER, GUEST
}

public record UserDTO(Long id, String name) {}

public class UserService implements UserRepository {
    private final Database db;
    
    public UserService(Database db) {
        this.db = db;
    }
    
    public Optional<User> findById(Long id) {
        return db.find(id);
    }
    
    public List<User> findAll() {
        return db.findAll();
    }
    
    private void validate(User user) {
        // private method
    }
}
        "#;
        let compressed = SymbolicCompressor::compress(content, "java");
        assert!(compressed.contains("package com.example.app"));
        assert!(compressed.contains("imports:"));
        assert!(compressed.contains("interface UserRepository"));
        assert!(compressed.contains("enum UserRole"));
        assert!(compressed.contains("record UserDTO"));
        assert!(compressed.contains("class UserService"));
        println!("Java compressed:\n{}", compressed);
    }

    #[test]
    fn test_compress_bash() {
        let content = r#"
#!/bin/bash

source ~/.bashrc
. /etc/profile

export PATH="/usr/local/bin:$PATH"
export EDITOR="vim"

alias ll="ls -la"
alias gs="git status"

setup_env() {
    echo "Setting up environment"
}

function deploy() {
    echo "Deploying..."
}

cleanup() {
    rm -rf /tmp/cache
}
        "#;
        let compressed = SymbolicCompressor::compress(content, "bash");
        assert!(compressed.contains("exports:"));
        assert!(compressed.contains("aliases:"));
        assert!(compressed.contains("setup_env()"));
        assert!(compressed.contains("deploy()"));
        println!("Bash compressed:\n{}", compressed);
    }
}
