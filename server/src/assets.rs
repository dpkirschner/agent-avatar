//! The kiosk page's files (`app/`), embedded by `build.rs`.
//!
//! The page is assembled from folders: `core/` (layout, speech, the
//! shared kit), `cards/<type>/` (one per card type), `status/<name>/`
//! (one per status template), `themes/<name>/` and `avatars/<name>/`.
//! Every card and status folder is linked into the page; one theme and
//! one avatar are, chosen by config.

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// An embedded file by its path under `app/`.
pub fn get(path: &str) -> Option<&'static [u8]> {
    FILES.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
}

/// The content type a file is served with; `None` keeps it private
/// (the page template, anything of an unknown kind).
pub fn content_type(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.')?.1;
    Some(match ext {
        "js" => "text/javascript",
        "css" => "text/css",
        "woff2" => "font/woff2",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "txt" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

/// The folders under `dir` that hold `file`, sorted: `names("cards",
/// "card.js")` is every card type the page can draw.
pub fn names(dir: &str, file: &str) -> Vec<&'static str> {
    FILES
        .iter()
        .filter_map(|(p, _)| {
            let rest = p.strip_prefix(dir)?.strip_prefix('/')?;
            let (name, f) = rest.split_once('/')?;
            (f == file).then_some(name)
        })
        .collect()
}

/// The `<link>` and `<script>` tags for the page's head: the layout,
/// every card's style, the theme last (so it wins), then the kit, every
/// card and status module, the avatar and the app. Scripts are `defer`,
/// so they run in this order once the page is parsed.
pub fn head(theme: &str, face: &str) -> String {
    let mut css = vec!["core/layout.css".to_string()];
    css.extend(
        names("cards", "card.css")
            .iter()
            .map(|n| format!("cards/{n}/card.css")),
    );
    css.push(format!("avatars/{face}/avatar.css"));
    css.push(format!("themes/{theme}/theme.css"));
    let mut js = vec!["core/kit.js".to_string()];
    js.extend(
        names("cards", "card.js")
            .iter()
            .map(|n| format!("cards/{n}/card.js")),
    );
    js.extend(
        names("status", "status.js")
            .iter()
            .map(|n| format!("status/{n}/status.js")),
    );
    js.push(format!("avatars/{face}/avatar.js"));
    js.push("core/app.js".to_string());
    let mut out = String::new();
    for h in css {
        out.push_str(&format!("<link rel=\"stylesheet\" href=\"{h}\">\n"));
    }
    for s in js {
        out.push_str(&format!("<script src=\"{s}\" defer></script>\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_folders_and_builds_the_head() {
        assert!(names("cards", "card.js").contains(&"chart"));
        assert!(names("themes", "theme.css").contains(&"picture-book"));
        assert_eq!(names("avatars", "avatar.js"), ["blob"]);
        let h = head("plain", "blob");
        let layout = h.find("core/layout.css").unwrap();
        let theme = h.find("themes/plain/theme.css").unwrap();
        let kit = h.find("core/kit.js").unwrap();
        let card = h.find("cards/text/card.js").unwrap();
        let app = h.find("core/app.js").unwrap();
        assert!(layout < theme && kit < card && card < app);
        for f in h
            .split('"')
            .filter(|s| s.ends_with(".js") || s.ends_with(".css"))
        {
            assert!(get(f).is_some(), "{f} is linked but not embedded");
        }
    }

    #[test]
    fn serves_only_known_kinds() {
        assert_eq!(content_type("core/app.js"), Some("text/javascript"));
        assert_eq!(content_type("index.html"), None);
        assert_eq!(
            content_type("themes/picture-book/fonts/fraunces.woff2"),
            Some("font/woff2")
        );
    }
}
