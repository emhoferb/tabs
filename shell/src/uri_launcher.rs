// SPDX-License-Identifier: GPL-2.0-or-later

//! Opening a URI with whatever the desktop has for it.
//!
//! Inside the Flatpak sandbox, `g_app_info_launch_default_for_uri()` goes
//! through the OpenURI portal, but by way of a `GFile` - and GVfs, which the
//! sandbox talks to, parses any scheme it does not know as if it were
//! hierarchical: `mailto:someone@example.org` comes back out of the `GFile`
//! as `mailto:///someone@example.org`, which no mail client accepts. The same
//! happens to `help:`, `tel:` and every other scheme without a `//`.
//!
//! So in the sandbox the portal is called directly, with the URI exactly as
//! the document wrote it. Local files still go through GLib, which hands the
//! portal an open file descriptor rather than a path the host may not see.

use crate::deps::*;

use std::path::Path;

pub async fn launch(uri: &str, context: Option<&gdk::AppLaunchContext>) -> Result<(), glib::Error> {
    if Path::new("/.flatpak-info").exists() && !uri.starts_with("file:") {
        return open_uri_portal(uri).await;
    }

    gio::AppInfo::launch_default_for_uri_future(uri, context).await
}

async fn open_uri_portal(uri: &str) -> Result<(), glib::Error> {
    let bus = gio::bus_get_future(gio::BusType::Session).await?;
    let parameters = glib::Variant::tuple_from_iter([
        // No parent window handle: the portal then centres any chooser it
        // shows on its own, which is all a missing handle costs.
        "".to_variant(),
        uri.to_variant(),
        glib::VariantDict::new(None).end(),
    ]);

    bus.call_future(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.OpenURI",
        "OpenURI",
        Some(&parameters),
        Some(glib::VariantTy::new("(o)").unwrap()),
        gio::DBusCallFlags::NONE,
        -1,
    )
    .await
    .map(|_| ())
}
