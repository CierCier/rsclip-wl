use std::cell::RefCell;
use std::collections::HashMap;

use rsclip_core::syntax::CodeLanguage;
use sourceview5::prelude::*;
use sourceview5::{Buffer, Language, LanguageManager, StyleScheme, StyleSchemeManager, View};

const RSCLIP_DARK_SCHEME_XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<style-scheme id="rsclip-dark" _name="rsclip Dark" version="1.0">
  <author>rsclip</author>
  <_description>Dark scheme for rsclip with transparent gutter and background</_description>
  <metadata>
    <property name="variant">dark</property>
  </metadata>

  <color name="blue_2" value="#62A0EA"/>
  <color name="blue_3" value="#3584E4"/>
  <color name="dark_1" value="#777777"/>
  <color name="dark_2" value="#6e6a86"/>
  <color name="green_2" value="#57E389"/>
  <color name="green_3" value="#33D17A"/>
  <color name="light_5" value="#dcd9e7"/>
  <color name="light_7" value="#9A9996"/>
  <color name="orange_2" value="#FFA348"/>
  <color name="orange_4" value="#E66100"/>
  <color name="red_1" value="#F66151"/>
  <color name="red_2" value="#ED333B"/>
  <color name="teal_2" value="#5BC8AF"/>
  <color name="teal_3" value="#33B2A4"/>
  <color name="violet_2" value="#7D8AC7"/>
  <color name="violet_4" value="#4E57BA"/>
  <color name="yellow_3" value="#F6D32D"/>

  <!-- Transparent backgrounds: no background attribute specified! -->
  <style name="text" foreground="light_5"/>
  <style name="line-numbers" foreground="dark_2"/>
  <style name="current-line-number" foreground="light_5" bold="true"/>
  <style name="cursor" foreground="light_5"/>
  <style name="bracket-match" bold="true"/>

  <style name="def:comment" foreground="dark_1" italic="true"/>
  <style name="def:doc-comment-element" foreground="light_7"/>
  <style name="def:constant" foreground="violet_2"/>
  <style name="def:string" foreground="teal_2"/>
  <style name="def:special-char" foreground="red_1"/>
  <style name="def:number" foreground="orange_2"/>
  <style name="def:floating-point" foreground="orange_2"/>
  <style name="def:decimal" foreground="orange_2"/>
  <style name="def:base-n-integer" foreground="orange_2"/>
  <style name="def:identifier" foreground="light_5"/>
  <style name="def:function" foreground="blue_2"/>
  <style name="def:type" foreground="teal_2" bold="true"/>
  <style name="def:statement" foreground="orange_2" bold="true"/>
  <style name="def:keyword" foreground="orange_2" bold="true"/>
  <style name="def:operator" foreground="orange_2"/>
  <style name="def:preprocessor" foreground="orange_4"/>
  <style name="def:boolean" foreground="violet_2"/>
  <style name="def:heading" foreground="teal_3" bold="true"/>
  <style name="def:inline-code" foreground="violet_2"/>
</style-scheme>
"##;

static INIT_SCHEME: std::sync::Once = std::sync::Once::new();

fn ensure_custom_scheme() {
    INIT_SCHEME.call_once(|| {
        let scheme_dir = std::env::temp_dir().join("rsclip-gtksourceview/styles");
        if std::fs::create_dir_all(&scheme_dir).is_ok() {
            let scheme_file = scheme_dir.join("rsclip-dark.xml");
            let _ = std::fs::write(&scheme_file, RSCLIP_DARK_SCHEME_XML);
            let sm = StyleSchemeManager::default();
            sm.append_search_path(&scheme_dir.to_string_lossy());
            sm.force_rescan();
        }
    });
}

// LanguageManager::default() and StyleSchemeManager::default() rescan their
// search paths on every call, and `lm.language(id)` re-resolves the language
// definition. setup_source_buffer ran all of that per preview render — tens of
// milliseconds during arrow-key navigation. The resolved singletons are frozen
// for the process lifetime, so resolve them once per thread and reuse.
thread_local! {
    static LANGUAGE_CACHE: RefCell<Option<HashMap<CodeLanguage, Language>>> =
        const { RefCell::new(None) };
    static SCHEME_CACHE: RefCell<Option<Option<StyleScheme>>> = const { RefCell::new(None) };
}

/// Cached sourceview language for `lang`, resolving on first use.
pub(crate) fn cached_language_for(lang: CodeLanguage) -> Option<Language> {
    LANGUAGE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let cache = cache.get_or_insert_with(HashMap::new);
        if let Some(language) = cache.get(&lang) {
            return Some(language.clone());
        }
        let language = LanguageManager::default().language(lang.sourceview_id());
        if let Some(language) = &language {
            cache.insert(lang, language.clone());
        }
        language
    })
}

fn cached_scheme() -> Option<StyleScheme> {
    SCHEME_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(scheme) = cache.as_ref().and_then(Option::as_ref) {
            return Some(scheme.clone());
        }
        ensure_custom_scheme();
        let sm = StyleSchemeManager::default();
        let scheme = sm
            .scheme("rsclip-dark")
            .or_else(|| sm.scheme("Adwaita-dark"))
            .or_else(|| sm.scheme("classic-dark"));
        *cache = Some(scheme.clone());
        scheme
    })
}

/// Sets up a `sourceview5::Buffer` with syntax highlighting and transparent dark style scheme for `lang`.
pub fn setup_source_buffer(code: &str, lang: CodeLanguage) -> Buffer {
    let buffer = Buffer::new(None);
    buffer.set_text(code);

    if let Some(language) = cached_language_for(lang) {
        buffer.set_language(Some(&language));
    }
    buffer.set_highlight_syntax(true);

    if let Some(scheme) = cached_scheme() {
        buffer.set_style_scheme(Some(&scheme));
    }

    buffer
}

/// Creates a read-only `sourceview5::View` configured for code preview with perfectly aligned line numbers.
pub fn create_source_view(buffer: &Buffer) -> View {
    let view = View::with_buffer(buffer);
    view.set_show_line_numbers(true);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_wrap_mode(gtk4::WrapMode::None);
    view.set_vexpand(true);
    view.set_hexpand(true);
    view.set_tab_width(4);
    view.set_top_margin(6);
    view.set_bottom_margin(6);
    view.set_left_margin(8);
    view.set_right_margin(8);
    view.set_highlight_current_line(false);
    view.add_css_class("sourceview-code");
    view
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GTK may only be initialized from one thread, and cargo runs each test
    /// on its own thread, so all GTK-backed checks share this single test.
    /// Without a display (CI), `gtk4::init` fails and the checks are skipped.
    #[test]
    fn sourceview_languages_scheme_and_buffer() {
        if gtk4::init().is_err() {
            return;
        }

        // Every CodeLanguage maps to a GtkSourceView language.
        {
            let lm = LanguageManager::default();
            let languages = [
                CodeLanguage::Rust,
                CodeLanguage::Python,
                CodeLanguage::JavaScript,
                CodeLanguage::TypeScript,
                CodeLanguage::Go,
                CodeLanguage::C,
                CodeLanguage::Cpp,
                CodeLanguage::CSharp,
                CodeLanguage::Java,
                CodeLanguage::Html,
                CodeLanguage::Css,
                CodeLanguage::Json,
                CodeLanguage::Yaml,
                CodeLanguage::Toml,
                CodeLanguage::Sql,
                CodeLanguage::Shell,
                CodeLanguage::Markdown,
                CodeLanguage::Php,
                CodeLanguage::Ruby,
                CodeLanguage::Lua,
                CodeLanguage::Xml,
                CodeLanguage::Diff,
                CodeLanguage::Docker,
            ];

            for lang in languages {
                let id = lang.sourceview_id();
                let source_lang = lm.language(id);
                assert!(
                    source_lang.is_some(),
                    "GtkSourceView failed to find language for {:?} (id: {})",
                    lang,
                    id
                );
            }
        }

        // The custom scheme exists and keeps line numbers transparent.
        {
            ensure_custom_scheme();
            let sm = StyleSchemeManager::default();
            let scheme = sm.scheme("rsclip-dark");
            assert!(
                scheme.is_some(),
                "Expected rsclip-dark style scheme to exist"
            );
            let scheme = scheme.unwrap();
            let ln = scheme.style("line-numbers");
            if let Some(ln) = ln {
                assert!(
                    !ln.is_background_set(),
                    "Line numbers background must not be set (transparent)"
                );
            }
        }

        // Buffers keep their text and enable highlighting.
        {
            let code = "def foo():\n    return 42\n";
            let buffer = setup_source_buffer(code, CodeLanguage::Python);
            let start = buffer.start_iter();
            let end = buffer.end_iter();
            assert_eq!(buffer.text(&start, &end, false), code);
            assert!(buffer.is_highlight_syntax());
        }
    }
}
