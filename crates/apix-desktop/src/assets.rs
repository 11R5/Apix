//! The asset source: gpui-kit's default icons plus the few extra Lucide
//! icons Apix uses (ISC licence, see assets/icons/LICENSE-LUCIDE). Shipping
//! the whole catalogue would add ~1500 SVGs to the binary for two icons.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

const EXTRA: &[(&str, &[u8])] = &[
    ("icons/lock.svg", include_bytes!("../assets/icons/lock.svg")),
    (
        "icons/lock-open.svg",
        include_bytes!("../assets/icons/lock-open.svg"),
    ),
];

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match EXTRA.iter().find(|(p, _)| *p == path) {
            Some((_, bytes)) => Ok(Some(Cow::Borrowed(bytes))),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out = gpui_kit::assets::Assets.list(path)?;
        out.extend(
            EXTRA
                .iter()
                .filter(|(p, _)| p.starts_with(path))
                .map(|(p, _)| SharedString::from(*p)),
        );
        Ok(out)
    }
}
