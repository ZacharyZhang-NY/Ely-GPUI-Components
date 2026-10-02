mod builtin;
mod locale;
mod skip;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use gpui::{App, Global, SharedString};

pub use locale::{Direction, LOCALES, Locale};
pub use skip::SkipLink;

type Catalog = HashMap<&'static str, &'static str>;

/// The locale shown and a catalog of messages per locale; every catalog holds the first one's keys.
pub struct I18n {
    locale: Locale,
    catalogs: Vec<(&'static str, Catalog)>,
}

impl Global for I18n {}

impl I18n {
    pub fn new(locale: &str) -> Self {
        Self {
            locale: Locale::of(locale),
            catalogs: Vec::new(),
        }
    }

    /// Adds a locale's messages; fails on a key the first catalog lacks or holds alone.
    pub fn catalog(mut self, tag: &str, messages: &[(&'static str, &'static str)]) -> Self {
        let locale = Locale::of(tag);
        let catalog: Catalog = messages.iter().copied().collect();
        assert_eq!(catalog.len(), messages.len(), "{tag} repeats a key");
        assert!(
            self.catalogs.iter().all(|(other, _)| *other != locale.tag),
            "{tag} has a catalog"
        );
        if let Some((first, source)) = self.catalogs.first() {
            let mut missing: Vec<_> = source
                .keys()
                .filter(|key| !catalog.contains_key(*key))
                .collect();
            let mut extra: Vec<_> = catalog
                .keys()
                .filter(|key| !source.contains_key(*key))
                .collect();
            missing.sort();
            extra.sort();
            assert!(missing.is_empty(), "{tag} misses {missing:?} from {first}");
            assert!(extra.is_empty(), "{tag} has {extra:?}, which {first} lacks");
        }
        self.catalogs.push((locale.tag, catalog));
        self
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// Shows another locale in every window; fails on one without a catalog.
    pub fn set_locale(tag: &str, cx: &mut App) {
        let locale = Locale::of(tag);
        let i18n = cx.global_mut::<I18n>();
        assert!(
            i18n.catalogs.iter().any(|(tag, _)| *tag == locale.tag),
            "no catalog for {}",
            locale.tag
        );
        log::info!("i18n: locale {} to {}", i18n.locale.tag, locale.tag);
        i18n.locale = locale;
        cx.refresh_windows();
    }

    /// The locale's own message for `key`, nothing when its catalog lacks it.
    pub(crate) fn find(&self, key: &str) -> Option<&'static str> {
        self.catalogs
            .iter()
            .find(|(tag, _)| *tag == self.locale.tag)
            .and_then(|(_, catalog)| catalog.get(key).copied())
    }

    /// The shown locale's message for `key`, each `{name}` filled from `args`; fails on a missing key or argument. gpui sets no paragraph direction, so a right-to-left message leads with a right-to-left mark: CoreText would lay out one that starts with Latin letters or a signed number left to right.
    pub fn text(&self, key: &str, args: &[(&str, &str)]) -> SharedString {
        let (_, catalog) = self
            .catalogs
            .iter()
            .find(|(tag, _)| *tag == self.locale.tag)
            .unwrap_or_else(|| panic!("no catalog for {}", self.locale.tag));
        let message = catalog
            .get(key)
            .unwrap_or_else(|| panic!("no message {key:?} in {}", self.locale.tag));
        let text = fill(message, args)
            .unwrap_or_else(|error| panic!("{key:?} in {}: {error}", self.locale.tag));
        match self.locale.direction {
            Direction::Ltr => text.into(),
            Direction::Rtl => format!("\u{200f}{text}").into(),
        }
    }
}

/// A component's message: the app's catalog first, Ely's builtin copy after. Without an `I18n` global it reads the English builtin.
pub fn text(cx: &App, key: &str, args: &[(&str, &str)]) -> SharedString {
    let (message, direction) = if cx.has_global::<I18n>() {
        let i18n = cx.global::<I18n>();
        let message = i18n
            .find(key)
            .unwrap_or_else(|| builtin::builtin(i18n.locale().tag, key));
        (message, i18n.locale().direction)
    } else {
        (builtin::builtin("en-US", key), Direction::Ltr)
    };
    let text = fill(message, args).unwrap_or_else(|error| panic!("{key:?}: {error}"));
    match direction {
        Direction::Ltr => text.into(),
        Direction::Rtl => format!("\u{200f}{text}").into(),
    }
}

/// `message` with each `{name}` replaced by its argument.
fn fill(message: &str, args: &[(&str, &str)]) -> Result<String, String> {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or("a { without }")?;
        let name = &after[..close];
        let (_, value) = args
            .iter()
            .find(|(arg, _)| *arg == name)
            .ok_or_else(|| format!("no argument {name:?}"))?;
        out.push_str(value);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
