use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported code languages for syntax detection, highlighting, and icons.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum CodeLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    C,
    Cpp,
    CSharp,
    Java,
    Html,
    Css,
    Json,
    Yaml,
    Toml,
    Sql,
    Shell,
    Markdown,
    Php,
    Ruby,
    Lua,
    Xml,
    Diff,
    Docker,
}

impl CodeLanguage {
    /// Human-friendly display label (e.g. "Rust", "Python").
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Go => "Go",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::CSharp => "C#",
            Self::Java => "Java",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Sql => "SQL",
            Self::Shell => "Shell",
            Self::Markdown => "Markdown",
            Self::Php => "PHP",
            Self::Ruby => "Ruby",
            Self::Lua => "Lua",
            Self::Xml => "XML",
            Self::Diff => "Diff",
            Self::Docker => "Dockerfile",
        }
    }

    /// Nerd font glyph for this language.
    pub fn nerd_icon(&self) -> &'static str {
        match self {
            Self::Rust => "\u{e7a8}",       //  nf-dev-rust
            Self::Python => "\u{e73c}",     //  nf-dev-python
            Self::JavaScript => "\u{e74e}", //  nf-dev-javascript
            Self::TypeScript => "\u{e628}", //  nf-seti-typescript
            Self::Go => "\u{e627}",         //  nf-seti-go
            Self::C => "\u{e61e}",          //  nf-custom-c
            Self::Cpp => "\u{e61d}",        //  nf-custom-cpp
            Self::CSharp => "\u{f031b}",    // 󰌛 nf-md-language_csharp
            Self::Java => "\u{e738}",       //  nf-dev-java
            Self::Html => "\u{e736}",       //  nf-dev-html5
            Self::Css => "\u{e749}",        //  nf-dev-css3
            Self::Json => "\u{e60b}",       //  nf-seti-json
            Self::Yaml => "\u{e601}",       //  nf-seti-yaml
            Self::Toml => "\u{e6b2}",       //  nf-custom-toml
            Self::Sql => "\u{e706}",        //  nf-dev-database
            Self::Shell => "\u{f489}",      //  nf-oct-terminal
            Self::Markdown => "\u{e73e}",   //  nf-dev-markdown
            Self::Php => "\u{e73d}",        //  nf-dev-php
            Self::Ruby => "\u{e739}",       //  nf-dev-ruby
            Self::Lua => "\u{e620}",        //  nf-seti-lua
            Self::Xml => "\u{e619}",        //  nf-seti-xml
            Self::Diff => "\u{e702}",       //  nf-dev-git
            Self::Docker => "\u{e7b0}",     //  nf-dev-docker
        }
    }

    /// Hex color associated with this language's branding/icon.
    pub fn nerd_color(&self) -> &'static str {
        match self {
            Self::Rust => "#dea584",
            Self::Python => "#3572A5",
            Self::JavaScript => "#f1e05a",
            Self::TypeScript => "#3178c6",
            Self::Go => "#00ADD8",
            Self::C => "#a8b9cc",
            Self::Cpp => "#f34b7d",
            Self::CSharp => "#178600",
            Self::Java => "#b07219",
            Self::Html => "#e34c26",
            Self::Css => "#563d7c",
            Self::Json => "#cbcb41",
            Self::Yaml => "#cb171e",
            Self::Toml => "#9c4221",
            Self::Sql => "#e38c00",
            Self::Shell => "#89e051",
            Self::Markdown => "#42a5f5",
            Self::Php => "#4F5D95",
            Self::Ruby => "#701516",
            Self::Lua => "#51a0cf",
            Self::Xml => "#0060ac",
            Self::Diff => "#41535b",
            Self::Docker => "#2496ed",
        }
    }

    /// GtkSourceView 5 language ID.
    pub fn sourceview_id(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python3",
            Self::JavaScript => "js",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "c-sharp",
            Self::Java => "java",
            Self::Html => "html",
            Self::Css => "css",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Sql => "sql",
            Self::Shell => "sh",
            Self::Markdown => "markdown",
            Self::Php => "php",
            Self::Ruby => "ruby",
            Self::Lua => "lua",
            Self::Xml => "xml",
            Self::Diff => "diff",
            Self::Docker => "docker",
        }
    }
}

impl fmt::Display for CodeLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Detect whether `text` represents code or structured markup, and identify its language.
///
/// Returns `None` if the text appears to be plain prose/notes or does not meet confidence thresholds.
pub fn detect_code_language(text: &str) -> Option<CodeLanguage> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Shebang line detection
    let first_line = trimmed.lines().next().unwrap_or("").trim();
    if let Some(shebang) = first_line.strip_prefix("#!") {
        let shebang_lower = shebang.to_ascii_lowercase();
        if shebang_lower.contains("python") {
            return Some(CodeLanguage::Python);
        }
        if shebang_lower.contains("bash")
            || shebang_lower.contains("/sh")
            || shebang_lower.contains("zsh")
            || shebang_lower.contains("dash")
        {
            return Some(CodeLanguage::Shell);
        }
        if shebang_lower.contains("node") {
            return Some(CodeLanguage::JavaScript);
        }
        if shebang_lower.contains("ruby") {
            return Some(CodeLanguage::Ruby);
        }
        if shebang_lower.contains("perl") {
            return Some(CodeLanguage::Shell);
        }
        if shebang_lower.contains("php") {
            return Some(CodeLanguage::Php);
        }
    }

    // 2. Diff / Git patch
    if trimmed.starts_with("diff --git ")
        || (trimmed.starts_with("--- ") && trimmed.contains("\n+++ "))
        || (trimmed.starts_with("@@ -") && trimmed.contains(" @@"))
    {
        return Some(CodeLanguage::Diff);
    }

    // 3. Dockerfile
    if first_line.starts_with("FROM ")
        || (trimmed.contains("\nFROM ")
            && (trimmed.contains("\nRUN ") || trimmed.contains("\nCOPY ")))
    {
        return Some(CodeLanguage::Docker);
    }

    // 4. PHP open tag
    if trimmed.starts_with("<?php") {
        return Some(CodeLanguage::Php);
    }

    // 5. XML / SVG / HTML doctype
    if first_line.starts_with("<?xml") {
        return Some(CodeLanguage::Xml);
    }
    let lower_first = first_line.to_ascii_lowercase();
    if lower_first.starts_with("<!doctype html")
        || lower_first.starts_with("<html")
        || trimmed.contains("<html")
        || (trimmed.contains("<div") && trimmed.contains("</div>"))
        || (trimmed.contains("<span") && trimmed.contains("</span>"))
    {
        return Some(CodeLanguage::Html);
    }

    // 6. JSON
    if ((trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']')))
        && trimmed.contains(':')
        && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    {
        return Some(CodeLanguage::Json);
    }

    // 7. TOML
    if (trimmed.starts_with('[') && trimmed.contains("]\n"))
        && toml::from_str::<toml::Value>(trimmed).is_ok()
        && trimmed.contains('=')
    {
        return Some(CodeLanguage::Toml);
    }

    // 8. SQL detection
    if detect_sql(trimmed) {
        return Some(CodeLanguage::Sql);
    }

    // 9. CSS detection
    if detect_css(trimmed) {
        return Some(CodeLanguage::Css);
    }

    // 10. Markdown detection
    if detect_markdown(trimmed) {
        return Some(CodeLanguage::Markdown);
    }

    // 11. Keyword and syntax scoring for programming languages
    score_programming_languages(trimmed)
}

fn detect_sql(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    let has_select_from = upper.contains("SELECT ") && upper.contains(" FROM ");
    let has_insert_into = upper.contains("INSERT INTO ") && upper.contains(" VALUES");
    let has_create_table = upper.contains("CREATE TABLE ");
    let has_update_set = upper.contains("UPDATE ") && upper.contains(" SET ");
    let has_alter_table = upper.contains("ALTER TABLE ");
    let has_delete_from = upper.contains("DELETE FROM ");

    if has_select_from
        || has_insert_into
        || has_create_table
        || has_update_set
        || has_alter_table
        || has_delete_from
    {
        // Require SQL terminator or syntax characters
        return text.contains(';') || text.contains('(') || text.contains('\n');
    }
    false
}

fn detect_css(text: &str) -> bool {
    if !text.contains('{') || !text.contains('}') {
        return false;
    }
    let properties = [
        "display:",
        "margin:",
        "padding:",
        "color:",
        "background:",
        "background-color:",
        "border:",
        "border-radius:",
        "font-size:",
        "font-family:",
        "box-sizing:",
        "align-items:",
        "justify-content:",
        "flex-direction:",
    ];
    let count = properties
        .iter()
        .filter(|prop| text.contains(*prop))
        .count();
    count >= 2
}

fn detect_markdown(text: &str) -> bool {
    let has_headers = text
        .lines()
        .any(|l| l.starts_with("# ") || l.starts_with("## ") || l.starts_with("### "));
    let has_fences = text.contains("```");
    let has_links = text.contains("](") && (text.contains("http://") || text.contains("https://"));
    let has_bullets = text
        .lines()
        .filter(|l| l.trim_start().starts_with("- ") || l.trim_start().starts_with("* "))
        .count()
        >= 2;

    (has_headers && (has_fences || has_links || has_bullets))
        || (has_fences && (has_links || has_bullets))
}

struct CodeTokens<'a> {
    tokens: std::collections::HashSet<&'a str>,
    has_c_comments: bool,
    has_hash_comments: bool,
    #[allow(dead_code)]
    has_dash_comments: bool,
    has_braces: bool,
    has_semicolons: bool,
    has_py_blocks: bool,
}

fn tokenize_code(text: &str) -> CodeTokens<'_> {
    let mut tokens = std::collections::HashSet::new();
    let mut has_c_comments = false;
    let mut has_hash_comments = false;
    let mut has_dash_comments = false;
    let mut semicolon_line_count = 0;
    let mut has_py_blocks = false;

    let has_braces = text.contains('{') && text.contains('}');

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            has_c_comments = true;
        } else if trimmed.starts_with('#')
            && !trimmed.starts_with("#!")
            && !trimmed.starts_with("#include")
            && !trimmed.starts_with("#define")
            && !trimmed.starts_with("#[")
        {
            has_hash_comments = true;
        } else if trimmed.starts_with("--") {
            has_dash_comments = true;
        }

        if trimmed.ends_with(';') {
            semicolon_line_count += 1;
        }

        if trimmed.ends_with(':') {
            let prefixes = [
                "def ", "class ", "if ", "elif ", "while ", "for ", "try:", "except", "with ",
                "else:", "finally:",
            ];
            if prefixes.iter().any(|p| trimmed.starts_with(p)) {
                has_py_blocks = true;
            }
        } else if trimmed.contains("else:")
            || (!has_braces
                && (trimmed.contains("while ") || trimmed.contains("elif "))
                && trimmed.contains(':'))
        {
            has_py_blocks = true;
        }

        let code_part = if let Some(idx) = trimmed.find("//") {
            &trimmed[..idx]
        } else if let Some(idx) = trimmed.find('#') {
            if trimmed.starts_with("#include")
                || trimmed.starts_with("#define")
                || trimmed.starts_with("#[")
            {
                trimmed
            } else {
                &trimmed[..idx]
            }
        } else {
            trimmed
        };

        for word in code_part.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
            if !word.is_empty() {
                tokens.insert(word);
            }
        }
    }

    CodeTokens {
        tokens,
        has_c_comments,
        has_hash_comments,
        has_dash_comments,
        has_braces,
        has_semicolons: semicolon_line_count >= 2,
        has_py_blocks,
    }
}

fn score_programming_languages(text: &str) -> Option<CodeLanguage> {
    let code_char_count = text
        .chars()
        .filter(|c| {
            matches!(
                c,
                '{' | '}' | ';' | '(' | ')' | '[' | ']' | '=' | '<' | '>' | ':' | '$' | '#'
            )
        })
        .count();
    if code_char_count == 0 {
        return None;
    }

    let parsed = tokenize_code(text);
    let t = &parsed.tokens;

    let mut rust_score = 0;
    let mut py_score = 0;
    let mut js_score = 0;
    let mut ts_score = 0;
    let mut go_score = 0;
    let mut c_score = 0;
    let mut cpp_score = 0;
    let mut cs_score = 0;
    let mut java_score = 0;
    let mut sh_score = 0;
    let mut ruby_score = 0;
    let mut lua_score = 0;

    // === RUST ===
    if !parsed.has_py_blocks && !t.contains("def") && !t.contains("elif") && !t.contains("package")
    {
        if t.contains("fn") {
            rust_score += 5;
        }
        if t.contains("pub") {
            rust_score += 4;
        }
        if t.contains("let") {
            rust_score += 3;
        }
        if t.contains("mut") {
            rust_score += 4;
        }
        if t.contains("impl") {
            rust_score += 5;
        }
        if t.contains("trait") {
            rust_score += 5;
        }
        if t.contains("struct") && (t.contains("pub") || parsed.has_braces) {
            rust_score += 4;
        }
        if t.contains("enum") && (t.contains("pub") || parsed.has_braces) {
            rust_score += 4;
        }
        if t.contains("match") && text.contains("=>") {
            rust_score += 5;
        }
        if t.contains("unsafe") {
            rust_score += 3;
        }
        let rust_types = [
            "usize", "u8", "u16", "u32", "u64", "u128", "isize", "i8", "i16", "i32", "i64", "bool",
        ];
        if rust_types.iter().any(|ty| t.contains(ty)) {
            rust_score += 4;
        }
        let rust_patterns = [
            "-> Option<",
            "-> Result<",
            "Some(",
            "Ok(",
            "Err(",
            "Vec<",
            "BinaryHeap::",
            "HashMap<",
            "Arc<",
            "Rc<",
            "Box<",
            "::",
        ];
        if rust_patterns.iter().any(|p| text.contains(p)) {
            rust_score += 4;
        }
        let rust_macros = [
            "println!",
            "eprintln!",
            "format!",
            "vec!",
            "panic!",
            "dbg!",
            "assert!",
        ];
        if rust_macros.iter().any(|m| text.contains(m)) {
            rust_score += 5;
        }
        if text.contains("use std::") || text.contains("use core::") || text.contains("use crate::")
        {
            rust_score += 5;
        }
        if text.contains("#[derive(") || text.contains("#[inline]") || text.contains("#[test]") {
            rust_score += 6;
        }
        if parsed.has_c_comments && rust_score > 0 {
            rust_score += 2;
        }
        if parsed.has_braces && rust_score > 0 {
            rust_score += 2;
        }
    }

    // === PYTHON ===
    let py_disqualified = parsed.has_c_comments
        || (parsed.has_braces
            && ["fn ", "pub ", "void ", "int ", "namespace ", "interface "]
                .iter()
                .any(|k| text.contains(k)))
        || (parsed.has_semicolons && parsed.has_braces);

    if !py_disqualified {
        if t.contains("def") {
            py_score += 6;
        }
        if t.contains("class") {
            py_score += 5;
        }
        if t.contains("elif") {
            py_score += 5;
        }
        if t.contains("import") || t.contains("from") {
            py_score += 2;
        }
        if t.contains("lambda") {
            py_score += 4;
        }
        if t.contains("self") {
            py_score += 3;
        }
        if t.contains("print") && text.contains('(') {
            py_score += 3;
        }
        if parsed.has_py_blocks {
            py_score += 6;
        }
        if text.contains("else:") {
            py_score += 4;
        }
        if text.contains("if __name__ == '__main__':")
            || text.contains("if __name__ == \"__main__\":")
        {
            py_score += 6;
        }
        if text.contains("is None") || text.contains("is not None") {
            py_score += 5;
        }
        if (t.contains("True") || t.contains("False") || t.contains("None")) && !parsed.has_braces {
            py_score += 3;
        }
        if ["int(input())", "input().split()", "sys.stdin", "bisect."]
            .iter()
            .any(|k| text.contains(k))
        {
            py_score += 5;
        }
        if (text.contains(" // ") || text.contains("// 2")) && !parsed.has_c_comments {
            py_score += 3;
        }
        if parsed.has_hash_comments && py_score > 0 {
            py_score += 2;
        }
    }

    // === GO ===
    if !parsed.has_py_blocks && !t.contains("def") && !t.contains("fn") {
        if t.contains("package") {
            go_score += 6;
        }
        if t.contains("func") {
            go_score += 5;
        }
        if t.contains("chan") {
            go_score += 5;
        }
        if t.contains("defer") {
            go_score += 5;
        }
        if text.contains(":=") {
            go_score += 5;
        }
        if text.contains("fmt.Println") || text.contains("fmt.Printf") {
            go_score += 5;
        }
        if t.contains("go") && t.contains("func") {
            go_score += 4;
        }
        if text.contains("struct {") && t.contains("type") {
            go_score += 5;
        }
    }

    // === TYPESCRIPT & JAVASCRIPT ===
    if !parsed.has_py_blocks && !t.contains("def") && !t.contains("fn") && !t.contains("package") {
        let has_js_var = t.contains("const") || t.contains("let") || t.contains("var");
        if has_js_var {
            js_score += 2;
            ts_score += 2;
        }
        if t.contains("function") {
            js_score += 4;
            ts_score += 4;
        }
        if text.contains("console.log(") {
            js_score += 5;
            ts_score += 5;
        }
        if t.contains("export") || t.contains("import") {
            js_score += 2;
            ts_score += 2;
        }
        if text.contains("=>") {
            js_score += 3;
            ts_score += 3;
        }
        if t.contains("interface") {
            ts_score += 6;
        }
        if [": string", ": number", ": boolean", ": any", "as const"]
            .iter()
            .any(|k| text.contains(k))
        {
            ts_score += 5;
        }
    }

    // === C & C++ ===
    if !parsed.has_py_blocks && !t.contains("def") && !t.contains("fn") && !t.contains("package") {
        if text.contains("#include <") || text.contains("#include \"") {
            c_score += 6;
            cpp_score += 6;
        }
        if text.contains("#define ") {
            c_score += 4;
            cpp_score += 4;
        }
        if text.contains("int main(") || text.contains("int main ()") {
            c_score += 5;
            cpp_score += 5;
        }
        if text.contains("printf(") {
            c_score += 4;
        }
        if text.contains("std::") {
            cpp_score += 6;
        }
        if text.contains("cout <<") || text.contains("cin >>") {
            cpp_score += 6;
        }
        if text.contains("template <") || text.contains("template<") {
            cpp_score += 6;
        }
        if t.contains("nullptr") {
            cpp_score += 5;
        }
    }

    // === JAVA & C# ===
    if !parsed.has_py_blocks && !t.contains("def") && !t.contains("fn") {
        if text.contains("public static void main") {
            java_score += 6;
            cs_score += 4;
        }
        if text.contains("System.out.println") {
            java_score += 6;
        }
        if text.contains("public class ") {
            java_score += 3;
            cs_score += 3;
        }
        if text.contains("using System;") || text.contains("namespace ") {
            cs_score += 6;
        }
        if text.contains("Console.WriteLine") {
            cs_score += 6;
        }
    }

    // === SHELL ===
    if !parsed.has_braces && !parsed.has_py_blocks {
        if text.contains("sudo ") || text.contains("chmod ") {
            sh_score += 4;
        }
        if text.contains("curl ") && (text.contains(" | ") || text.contains(" -")) {
            sh_score += 4;
        }
        if text.contains("export ") && text.contains('=') {
            sh_score += 4;
        }
        if text.contains("if [ ") && (text.contains("then") || text.contains("fi")) {
            sh_score += 5;
        }
        if text.contains("echo ") {
            sh_score += 2;
        }
    }

    // === RUBY ===
    if !parsed.has_c_comments && !parsed.has_braces {
        if text.contains("def ") && text.contains("end") {
            ruby_score += 5;
        }
        if text.contains("attr_accessor ") {
            ruby_score += 5;
        }
        if text.contains("puts ") {
            ruby_score += 3;
        }
    }

    // === LUA ===
    if !parsed.has_c_comments && !parsed.has_py_blocks {
        if text.contains("local ") && (text.contains("function") || text.contains('=')) {
            lua_score += 5;
        }
        if text.contains("nil") && text.contains("then") {
            lua_score += 4;
        }
    }

    let mut scores = [
        (rust_score, CodeLanguage::Rust),
        (py_score, CodeLanguage::Python),
        (ts_score, CodeLanguage::TypeScript),
        (js_score, CodeLanguage::JavaScript),
        (go_score, CodeLanguage::Go),
        (cpp_score, CodeLanguage::Cpp),
        (c_score, CodeLanguage::C),
        (java_score, CodeLanguage::Java),
        (cs_score, CodeLanguage::CSharp),
        (sh_score, CodeLanguage::Shell),
        (ruby_score, CodeLanguage::Ruby),
        (lua_score, CodeLanguage::Lua),
    ];

    scores.sort_by_key(|b| std::cmp::Reverse(b.0));
    let (top_score, winner) = scores[0];
    let (second_score, runner_up) = scores[1];

    if top_score < 4 {
        return None;
    }

    if top_score == second_score && top_score > 0 {
        if (winner == CodeLanguage::TypeScript && runner_up == CodeLanguage::JavaScript)
            || (winner == CodeLanguage::JavaScript && runner_up == CodeLanguage::TypeScript)
        {
            return if ts_score > 0 && ts_score >= js_score {
                Some(CodeLanguage::TypeScript)
            } else {
                Some(CodeLanguage::JavaScript)
            };
        }
        if (winner == CodeLanguage::Cpp && runner_up == CodeLanguage::C)
            || (winner == CodeLanguage::C && runner_up == CodeLanguage::Cpp)
        {
            return if cpp_score > c_score {
                Some(CodeLanguage::Cpp)
            } else {
                Some(CodeLanguage::C)
            };
        }
        return None;
    }

    if top_score < 7 && (top_score - second_score) < 2 {
        return None;
    }

    Some(winner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_shebang_shell() {
        let snippet = "#!/bin/bash\necho 'hello world'";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Shell));
    }

    #[test]
    fn detects_shebang_python() {
        let snippet = "#!/usr/bin/env python3\nprint('hello')";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Python));
    }

    #[test]
    fn detects_rust_syntax() {
        let snippet = "fn main() {\n    let mut count = 0;\n    println!(\"count: {count}\");\n}";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Rust));
    }

    #[test]
    fn detects_python_syntax() {
        let snippet = "def calculate_total(items):\n    return sum(item.price for item in items)";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Python));
    }

    #[test]
    fn detects_go_syntax() {
        let snippet =
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n    x := 42\n    fmt.Println(x)\n}";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Go));
    }

    #[test]
    fn detects_typescript_syntax() {
        let snippet = "interface UserProfile {\n    id: string;\n    active: boolean;\n}\nexport const getUser = async (id: string) => {\n    console.log(id);\n};";
        assert_eq!(
            detect_code_language(snippet),
            Some(CodeLanguage::TypeScript)
        );
    }

    #[test]
    fn detects_c_and_cpp_syntax() {
        let c_snippet = "#include <stdio.h>\n\nint main() {\n    printf(\"Hello, world!\\n\");\n    return 0;\n}";
        assert!(matches!(
            detect_code_language(c_snippet),
            Some(CodeLanguage::C) | Some(CodeLanguage::Cpp)
        ));

        let cpp_snippet = "#include <iostream>\n\nint main() {\n    std::cout << \"Hello\" << std::endl;\n    return 0;\n}";
        assert_eq!(detect_code_language(cpp_snippet), Some(CodeLanguage::Cpp));
    }

    #[test]
    fn detects_json() {
        let snippet =
            "{\n  \"name\": \"rsclip\",\n  \"version\": \"0.1.17\",\n  \"active\": true\n}";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Json));
    }

    #[test]
    fn detects_html() {
        let snippet = "<!DOCTYPE html>\n<html>\n<body>\n  <div class=\"container\">\n    <p>Hello world</p>\n  </div>\n</body>\n</html>";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Html));
    }

    #[test]
    fn detects_diff() {
        let snippet = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Diff));
    }

    #[test]
    fn detects_dockerfile() {
        let snippet = "FROM rust:1.95-alpine\nWORKDIR /app\nCOPY . .\nRUN cargo build --release\nCMD [\"./target/release/rsclip\"]";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Docker));
    }

    #[test]
    fn detects_binary_search_python_snippet() {
        let snippet = "low = 0\nhigh = n - 1\n\nwhile low < high:\n    mid = (low + high) // 2\n    if nums[mid] > nums[high]:\n        low = mid + 1\n    else:\n        high = mid\n\nprint(low)";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Python));

        let preview = "low = 0 high = n - 1 while low < high: mid = (low + high) // 2 if nums[mid] > nums[high]: low = mid + 1 else: high = mid print(low)";
        assert_eq!(detect_code_language(preview), Some(CodeLanguage::Python));
    }

    #[test]
    fn detects_dijkstra_rust_snippet() {
        let snippet = r#"// Dijkstra algorithm in Rust
pub fn shortest_path(graph: &Graph, start: usize, goal: usize) -> Option<u32> {
    let mut dist: Vec<_> = (0..graph.len()).map(|_| u32::MAX).collect();
    let mut heap = BinaryHeap::new();

    dist[start] = 0;
    heap.push(State { cost: 0, position: start });

    while let Some(State { cost, position }) = heap.pop() {
        if position == goal { return Some(cost); }
        if cost > dist[position] { continue; }

        for edge in &graph.edges[position] {
            let next = State { cost: cost + edge.cost, position: edge.node };
            if next.cost < dist[next.position] {
                heap.push(next);
                dist[next.position] = next.cost;
            }
        }
    }
    None
}"#;
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Rust));
    }

    #[test]
    fn plain_text_is_not_detected_as_code() {
        let prose = "Hey Radhey, make sure we meet at 3pm today for the design review. Let me know if that works!";
        assert_eq!(detect_code_language(prose), None);

        let random_sentence = "This is a simple note without any code or brackets or keywords.";
        assert_eq!(detect_code_language(random_sentence), None);

        let sentence_with_keywords = "Let me know if you can meet while I am in town for the conference. We can talk about rust.";
        assert_eq!(detect_code_language(sentence_with_keywords), None);
    }

    #[test]
    fn c_comments_disqualify_python() {
        let snippet = "// This is a C++ or Rust file\nint x = 10;\nwhile (x > 0) {\n    x--;\n}";
        assert_ne!(detect_code_language(snippet), Some(CodeLanguage::Python));
    }

    #[test]
    fn short_rust_function_detected() {
        let snippet = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Rust));
    }

    #[test]
    fn short_python_function_detected() {
        let snippet = "def add(a, b):\n    return a + b";
        assert_eq!(detect_code_language(snippet), Some(CodeLanguage::Python));
    }
}
